"""Aggregated analytics endpoints for the dashboard."""

from fastapi import APIRouter, Depends
from sqlalchemy import select, func, case
from sqlalchemy.ext.asyncio import AsyncSession

from backend.database.connection import get_db
from backend.database.models import (
    Campaign, Post, MetricSnapshot, Product, AIAnalysis,
)

router = APIRouter(prefix="/api/analytics", tags=["analytics"])


@router.get("/overview")
async def overview(db: AsyncSession = Depends(get_db)):
    """Dashboard summary: total campaigns, posts, top posts, etc."""
    campaign_count = (await db.execute(
        select(func.count(Campaign.id)).where(Campaign.status != "archived")
    )).scalar() or 0

    post_count = (await db.execute(select(func.count(Post.id)))).scalar() or 0
    product_count = (await db.execute(select(func.count(Product.id)))).scalar() or 0

    total_metrics = await db.execute(
        select(
            func.coalesce(func.sum(MetricSnapshot.views), 0),
            func.coalesce(func.sum(MetricSnapshot.likes), 0),
            func.coalesce(func.sum(MetricSnapshot.comments), 0),
            func.coalesce(func.sum(MetricSnapshot.shares), 0),
        )
    )
    m = total_metrics.first()

    # Top 5 posts by likes
    top_posts_result = await db.execute(
        select(
            Post.id,
            Post.title,
            Post.platform,
            func.coalesce(func.sum(MetricSnapshot.likes), 0).label("total_likes"),
        )
        .outerjoin(MetricSnapshot, MetricSnapshot.post_id == Post.id)
        .group_by(Post.id)
        .order_by(func.coalesce(func.sum(MetricSnapshot.likes), 0).desc())
        .limit(5)
    )
    top_posts = [
        {"id": r[0], "title": r[1], "platform": r[2], "total_likes": r[3]}
        for r in top_posts_result.all()
    ]

    return {
        "active_campaigns": campaign_count,
        "total_posts": post_count,
        "total_products": product_count,
        "total_views": m[0],
        "total_likes": m[1],
        "total_comments": m[2],
        "total_shares": m[3],
        "top_posts": top_posts,
    }


@router.get("/platforms")
async def platforms(db: AsyncSession = Depends(get_db)):
    """Engagement breakdown by platform."""
    result = await db.execute(
        select(
            Post.platform,
            func.count(Post.id).label("post_count"),
            func.coalesce(func.sum(MetricSnapshot.views), 0).label("views"),
            func.coalesce(func.sum(MetricSnapshot.likes), 0).label("likes"),
            func.coalesce(func.sum(MetricSnapshot.comments), 0).label("comments"),
            func.coalesce(func.sum(MetricSnapshot.shares), 0).label("shares"),
        )
        .outerjoin(MetricSnapshot, MetricSnapshot.post_id == Post.id)
        .group_by(Post.platform)
        .order_by(func.count(Post.id).desc())
    )
    return [
        {
            "platform": r[0],
            "post_count": r[1],
            "views": r[2],
            "likes": r[3],
            "comments": r[4],
            "shares": r[5],
        }
        for r in result.all()
    ]


@router.get("/post-types")
async def post_types(db: AsyncSession = Depends(get_db)):
    """Engagement breakdown by post type."""
    result = await db.execute(
        select(
            Post.post_type,
            func.count(Post.id).label("post_count"),
            func.coalesce(func.sum(MetricSnapshot.views), 0).label("views"),
            func.coalesce(func.sum(MetricSnapshot.likes), 0).label("likes"),
            func.coalesce(func.sum(MetricSnapshot.comments), 0).label("comments"),
            func.coalesce(func.sum(MetricSnapshot.shares), 0).label("shares"),
        )
        .outerjoin(MetricSnapshot, MetricSnapshot.post_id == Post.id)
        .group_by(Post.post_type)
        .order_by(func.count(Post.id).desc())
    )
    return [
        {
            "post_type": r[0],
            "post_count": r[1],
            "views": r[2],
            "likes": r[3],
            "comments": r[4],
            "shares": r[5],
        }
        for r in result.all()
    ]


@router.get("/trends")
async def trends(db: AsyncSession = Depends(get_db)):
    """Time-series engagement across all campaigns."""
    result = await db.execute(
        select(
            MetricSnapshot.snapshot_date,
            func.coalesce(func.sum(MetricSnapshot.views), 0).label("views"),
            func.coalesce(func.sum(MetricSnapshot.likes), 0).label("likes"),
            func.coalesce(func.sum(MetricSnapshot.comments), 0).label("comments"),
            func.coalesce(func.sum(MetricSnapshot.shares), 0).label("shares"),
            func.count(MetricSnapshot.id).label("snapshot_count"),
        )
        .group_by(MetricSnapshot.snapshot_date)
        .order_by(MetricSnapshot.snapshot_date.asc())
    )
    return [
        {
            "date": str(r[0]),
            "views": r[1],
            "likes": r[2],
            "comments": r[3],
            "shares": r[4],
            "snapshot_count": r[5],
        }
        for r in result.all()
    ]
