"""Metric snapshots and fetch trigger endpoints."""

import logging
from datetime import datetime, date

from fastapi import APIRouter, Depends, HTTPException, BackgroundTasks
from pydantic import BaseModel
from sqlalchemy import select, func
from sqlalchemy.ext.asyncio import AsyncSession

from backend.database.connection import get_db, AsyncSessionLocal
from backend.database.models import MetricSnapshot, Post, Campaign, SystemState
from backend.services.metric_collector import MetricCollector

logger = logging.getLogger(__name__)

router = APIRouter(tags=["metrics"])


class MetricSnapshotResponse(BaseModel):
    id: int
    post_id: str
    snapshot_date: date
    views: int = 0
    impressions: int = 0
    likes: int = 0
    dislikes: int = 0
    comments: int = 0
    shares: int = 0
    saves: int = 0
    clicks: int = 0
    watch_time_seconds: int | None = None
    followers_gained: int = 0
    custom_metrics: str | None = None
    fetched_via: str = "manual"
    created_at: datetime | None = None

    model_config = {"from_attributes": True}


class AggregatedMetrics(BaseModel):
    campaign_id: str
    total_views: int = 0
    total_impressions: int = 0
    total_likes: int = 0
    total_dislikes: int = 0
    total_comments: int = 0
    total_shares: int = 0
    total_saves: int = 0
    total_clicks: int = 0
    post_count: int = 0
    snapshot_count: int = 0


# Fetch state tracked in memory
_fetch_state = {"running": False, "last_started": None, "last_completed": None, "error": None}

# Shared collector instance
_collector = MetricCollector()


async def _run_fetch_background():
    """Run metric collection in a background task."""
    _fetch_state["running"] = True
    _fetch_state["last_started"] = datetime.utcnow().isoformat()
    _fetch_state["error"] = None

    try:
        async with AsyncSessionLocal() as session:
            report = await _collector.collect_all(session)

        _fetch_state["last_completed"] = datetime.utcnow().isoformat()
        logger.info(
            "Background fetch completed: fetched=%d, failed=%d, skipped=%d",
            report.fetched, report.failed, report.skipped,
        )
        if report.errors:
            _fetch_state["error"] = f"{report.failed} failures: {'; '.join(report.errors[:5])}"
    except Exception as e:
        _fetch_state["error"] = str(e)
        logger.error("Background fetch exception: %s", e)
    finally:
        _fetch_state["running"] = False


@router.get(
    "/api/posts/{post_id}/metrics", response_model=list[MetricSnapshotResponse]
)
async def get_post_metrics(post_id: str, db: AsyncSession = Depends(get_db)):
    """Time-series metric snapshots for a post."""
    result = await db.execute(select(Post).where(Post.id == post_id))
    if not result.scalar_one_or_none():
        raise HTTPException(status_code=404, detail="Post not found")

    result = await db.execute(
        select(MetricSnapshot)
        .where(MetricSnapshot.post_id == post_id)
        .order_by(MetricSnapshot.snapshot_date.asc())
    )
    return result.scalars().all()


@router.get(
    "/api/campaigns/{campaign_id}/metrics", response_model=AggregatedMetrics
)
async def get_campaign_metrics(
    campaign_id: str, db: AsyncSession = Depends(get_db)
):
    """Aggregated metrics for all posts in a campaign."""
    result = await db.execute(select(Campaign).where(Campaign.id == campaign_id))
    if not result.scalar_one_or_none():
        raise HTTPException(status_code=404, detail="Campaign not found")

    metrics_result = await db.execute(
        select(
            func.coalesce(func.sum(MetricSnapshot.views), 0),
            func.coalesce(func.sum(MetricSnapshot.impressions), 0),
            func.coalesce(func.sum(MetricSnapshot.likes), 0),
            func.coalesce(func.sum(MetricSnapshot.dislikes), 0),
            func.coalesce(func.sum(MetricSnapshot.comments), 0),
            func.coalesce(func.sum(MetricSnapshot.shares), 0),
            func.coalesce(func.sum(MetricSnapshot.saves), 0),
            func.coalesce(func.sum(MetricSnapshot.clicks), 0),
            func.count(MetricSnapshot.id),
        )
        .join(Post, MetricSnapshot.post_id == Post.id)
        .where(Post.campaign_id == campaign_id)
    )
    row = metrics_result.first()

    post_count_result = await db.execute(
        select(func.count(Post.id)).where(Post.campaign_id == campaign_id)
    )
    post_count = post_count_result.scalar() or 0

    return AggregatedMetrics(
        campaign_id=campaign_id,
        total_views=row[0],
        total_impressions=row[1],
        total_likes=row[2],
        total_dislikes=row[3],
        total_comments=row[4],
        total_shares=row[5],
        total_saves=row[6],
        total_clicks=row[7],
        snapshot_count=row[8],
        post_count=post_count,
    )


@router.post("/api/metrics/fetch")
async def trigger_fetch(background_tasks: BackgroundTasks):
    """Trigger manual metric fetch across all platforms."""
    if _fetch_state["running"]:
        return {"message": "Fetch already in progress", "status": "running"}

    background_tasks.add_task(_run_fetch_background)
    return {"message": "Metric fetch started", "status": "started"}


@router.get("/api/metrics/fetch/status")
async def fetch_status():
    """Check if a fetch is currently running."""
    return {
        "running": _fetch_state["running"],
        "last_started": _fetch_state["last_started"],
        "last_completed": _fetch_state["last_completed"],
        "error": _fetch_state["error"],
    }
