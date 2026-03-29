"""CRUD endpoints for campaigns."""

from uuid import uuid4
from datetime import datetime, date
from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel
from sqlalchemy import select, func
from sqlalchemy.ext.asyncio import AsyncSession
from sqlalchemy.orm import selectinload

from backend.database.connection import get_db
from backend.database.models import Campaign, Post, MetricSnapshot

router = APIRouter(prefix="/api/campaigns", tags=["campaigns"])


class CampaignCreate(BaseModel):
    product_id: str
    name: str
    status: str = "active"
    goal: str | None = None
    target_audience: str | None = None
    start_date: date | None = None
    end_date: date | None = None
    notes: str | None = None


class CampaignUpdate(BaseModel):
    product_id: str | None = None
    name: str | None = None
    status: str | None = None
    goal: str | None = None
    target_audience: str | None = None
    start_date: date | None = None
    end_date: date | None = None
    notes: str | None = None


class CampaignResponse(BaseModel):
    id: str
    product_id: str
    name: str
    status: str | None = None
    goal: str | None = None
    target_audience: str | None = None
    start_date: date | None = None
    end_date: date | None = None
    notes: str | None = None
    created_at: datetime | None = None
    updated_at: datetime | None = None
    post_count: int = 0
    total_likes: int = 0
    total_comments: int = 0
    total_views: int = 0

    model_config = {"from_attributes": True}


class CampaignDetailResponse(CampaignResponse):
    posts: list[dict] = []


@router.get("", response_model=list[CampaignResponse])
async def list_campaigns(db: AsyncSession = Depends(get_db)):
    result = await db.execute(
        select(Campaign).order_by(Campaign.created_at.desc())
    )
    campaigns = result.scalars().all()

    response = []
    for campaign in campaigns:
        # Get post count
        post_count_result = await db.execute(
            select(func.count(Post.id)).where(Post.campaign_id == campaign.id)
        )
        post_count = post_count_result.scalar() or 0

        # Get aggregated metrics from the latest snapshot per post
        metrics_result = await db.execute(
            select(
                func.coalesce(func.sum(MetricSnapshot.likes), 0),
                func.coalesce(func.sum(MetricSnapshot.comments), 0),
                func.coalesce(func.sum(MetricSnapshot.views), 0),
            )
            .join(Post, MetricSnapshot.post_id == Post.id)
            .where(Post.campaign_id == campaign.id)
        )
        metrics = metrics_result.first()

        resp = CampaignResponse.model_validate(campaign)
        resp.post_count = post_count
        resp.total_likes = metrics[0] if metrics else 0
        resp.total_comments = metrics[1] if metrics else 0
        resp.total_views = metrics[2] if metrics else 0
        response.append(resp)

    return response


@router.get("/{campaign_id}", response_model=CampaignDetailResponse)
async def get_campaign(campaign_id: str, db: AsyncSession = Depends(get_db)):
    result = await db.execute(
        select(Campaign)
        .options(selectinload(Campaign.posts))
        .where(Campaign.id == campaign_id)
    )
    campaign = result.unique().scalar_one_or_none()
    if not campaign:
        raise HTTPException(status_code=404, detail="Campaign not found")

    posts_data = [
        {
            "id": p.id,
            "platform": p.platform,
            "post_type": p.post_type,
            "title": p.title,
            "url": p.url,
            "target_community": p.target_community,
            "posted_at": p.posted_at.isoformat() if p.posted_at else None,
            "is_api_tracked": p.is_api_tracked,
            "created_at": p.created_at.isoformat() if p.created_at else None,
        }
        for p in campaign.posts
    ]

    return CampaignDetailResponse(
        id=campaign.id,
        product_id=campaign.product_id,
        name=campaign.name,
        status=campaign.status,
        goal=campaign.goal,
        target_audience=campaign.target_audience,
        start_date=campaign.start_date,
        end_date=campaign.end_date,
        notes=campaign.notes,
        created_at=campaign.created_at,
        updated_at=campaign.updated_at,
        post_count=len(campaign.posts),
        posts=posts_data,
    )


@router.post("", response_model=CampaignResponse, status_code=201)
async def create_campaign(data: CampaignCreate, db: AsyncSession = Depends(get_db)):
    campaign = Campaign(id=str(uuid4()), **data.model_dump())
    db.add(campaign)
    await db.commit()
    await db.refresh(campaign)
    return CampaignResponse.model_validate(campaign)


@router.put("/{campaign_id}", response_model=CampaignResponse)
async def update_campaign(
    campaign_id: str, data: CampaignUpdate, db: AsyncSession = Depends(get_db)
):
    result = await db.execute(select(Campaign).where(Campaign.id == campaign_id))
    campaign = result.scalar_one_or_none()
    if not campaign:
        raise HTTPException(status_code=404, detail="Campaign not found")
    for key, value in data.model_dump(exclude_unset=True).items():
        setattr(campaign, key, value)
    campaign.updated_at = datetime.utcnow()
    await db.commit()
    await db.refresh(campaign)
    return CampaignResponse.model_validate(campaign)


@router.delete("/{campaign_id}", status_code=200)
async def delete_campaign(campaign_id: str, db: AsyncSession = Depends(get_db)):
    """Soft delete: set status to 'archived'."""
    result = await db.execute(select(Campaign).where(Campaign.id == campaign_id))
    campaign = result.scalar_one_or_none()
    if not campaign:
        raise HTTPException(status_code=404, detail="Campaign not found")
    campaign.status = "archived"
    campaign.updated_at = datetime.utcnow()
    await db.commit()
    return {"message": "Campaign archived", "id": campaign_id}
