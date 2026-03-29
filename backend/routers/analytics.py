"""Aggregated analytics endpoints for the dashboard."""

from typing import Optional
from fastapi import APIRouter, Depends, Query
from sqlalchemy import select, func, case
from sqlalchemy.ext.asyncio import AsyncSession

from backend.database.connection import get_db
from backend.database.models import (
    Campaign, Post, MetricSnapshot, Product, AIAnalysis,
)

router = APIRouter(prefix="/api/analytics", tags=["analytics"])


@router.get("/overview")
async def overview(
    profile_id: Optional[str] = Query(None),
    db: AsyncSession = Depends(get_db),
):
    """Dashboard summary: total campaigns, posts, top posts, etc."""
    campaign_query = select(func.count(Campaign.id)).where(Campaign.status != "archived")
    if profile_id is not None:
        campaign_query = campaign_query.where(Campaign.profile_id == profile_id)
    campaign_count = (await db.execute(campaign_query)).scalar() or 0

    post_query = select(func.count(Post.id))
    if profile_id is not None:
        post_query = post_query.join(Campaign, Post.campaign_id == Campaign.id).where(Campaign.profile_id == profile_id)
    post_count = (await db.execute(post_query)).scalar() or 0

    product_query = select(func.count(Product.id))
    if profile_id is not None:
        product_query = product_query.where(Product.profile_id == profile_id)
    product_count = (await db.execute(product_query)).scalar() or 0

    metrics_query = select(
        func.coalesce(func.sum(MetricSnapshot.views), 0),
        func.coalesce(func.sum(MetricSnapshot.likes), 0),
        func.coalesce(func.sum(MetricSnapshot.comments), 0),
        func.coalesce(func.sum(MetricSnapshot.shares), 0),
    )
    if profile_id is not None:
        metrics_query = (
            metrics_query
            .join(Post, MetricSnapshot.post_id == Post.id)
            .join(Campaign, Post.campaign_id == Campaign.id)
            .where(Campaign.profile_id == profile_id)
        )
    total_metrics = await db.execute(metrics_query)
    m = total_metrics.first()

    # Top 5 posts by likes
    top_posts_query = (
        select(
            Post.id,
            Post.title,
            Post.platform,
            func.coalesce(func.sum(MetricSnapshot.likes), 0).label("total_likes"),
        )
        .outerjoin(MetricSnapshot, MetricSnapshot.post_id == Post.id)
    )
    if profile_id is not None:
        top_posts_query = (
            top_posts_query
            .join(Campaign, Post.campaign_id == Campaign.id)
            .where(Campaign.profile_id == profile_id)
        )
    top_posts_query = (
        top_posts_query
        .group_by(Post.id)
        .order_by(func.coalesce(func.sum(MetricSnapshot.likes), 0).desc())
        .limit(5)
    )
    top_posts_result = await db.execute(top_posts_query)
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
async def platforms(
    profile_id: Optional[str] = Query(None),
    db: AsyncSession = Depends(get_db),
):
    """Engagement breakdown by platform."""
    query = (
        select(
            Post.platform,
            func.count(Post.id).label("post_count"),
            func.coalesce(func.sum(MetricSnapshot.views), 0).label("views"),
            func.coalesce(func.sum(MetricSnapshot.likes), 0).label("likes"),
            func.coalesce(func.sum(MetricSnapshot.comments), 0).label("comments"),
            func.coalesce(func.sum(MetricSnapshot.shares), 0).label("shares"),
        )
        .outerjoin(MetricSnapshot, MetricSnapshot.post_id == Post.id)
    )
    if profile_id is not None:
        query = query.join(Campaign, Post.campaign_id == Campaign.id).where(Campaign.profile_id == profile_id)
    query = query.group_by(Post.platform).order_by(func.count(Post.id).desc())
    result = await db.execute(query)
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
async def post_types(
    profile_id: Optional[str] = Query(None),
    db: AsyncSession = Depends(get_db),
):
    """Engagement breakdown by post type."""
    query = (
        select(
            Post.post_type,
            func.count(Post.id).label("post_count"),
            func.coalesce(func.sum(MetricSnapshot.views), 0).label("views"),
            func.coalesce(func.sum(MetricSnapshot.likes), 0).label("likes"),
            func.coalesce(func.sum(MetricSnapshot.comments), 0).label("comments"),
            func.coalesce(func.sum(MetricSnapshot.shares), 0).label("shares"),
        )
        .outerjoin(MetricSnapshot, MetricSnapshot.post_id == Post.id)
    )
    if profile_id is not None:
        query = query.join(Campaign, Post.campaign_id == Campaign.id).where(Campaign.profile_id == profile_id)
    query = query.group_by(Post.post_type).order_by(func.count(Post.id).desc())
    result = await db.execute(query)
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
async def trends(
    profile_id: Optional[str] = Query(None),
    db: AsyncSession = Depends(get_db),
):
    """Time-series engagement across all campaigns."""
    query = select(
        MetricSnapshot.snapshot_date,
        func.coalesce(func.sum(MetricSnapshot.views), 0).label("views"),
        func.coalesce(func.sum(MetricSnapshot.likes), 0).label("likes"),
        func.coalesce(func.sum(MetricSnapshot.comments), 0).label("comments"),
        func.coalesce(func.sum(MetricSnapshot.shares), 0).label("shares"),
        func.count(MetricSnapshot.id).label("snapshot_count"),
    )
    if profile_id is not None:
        query = (
            query
            .join(Post, MetricSnapshot.post_id == Post.id)
            .join(Campaign, Post.campaign_id == Campaign.id)
            .where(Campaign.profile_id == profile_id)
        )
    query = query.group_by(MetricSnapshot.snapshot_date).order_by(MetricSnapshot.snapshot_date.asc())
    result = await db.execute(query)
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
