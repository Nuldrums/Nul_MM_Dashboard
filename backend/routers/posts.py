"""CRUD endpoints for posts."""

from uuid import uuid4
from datetime import datetime
from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from backend.database.connection import get_db
from backend.database.models import Post, Campaign

router = APIRouter(tags=["posts"])


class PostCreate(BaseModel):
    platform: str
    post_type: str
    platform_post_id: str | None = None
    url: str | None = None
    title: str | None = None
    body_preview: str | None = None
    target_community: str | None = None
    posted_at: datetime | None = None
    tags: str | None = None
    is_api_tracked: int = 0


class PostUpdate(BaseModel):
    platform: str | None = None
    post_type: str | None = None
    platform_post_id: str | None = None
    url: str | None = None
    title: str | None = None
    body_preview: str | None = None
    target_community: str | None = None
    posted_at: datetime | None = None
    tags: str | None = None
    is_api_tracked: int | None = None


class PostResponse(BaseModel):
    id: str
    campaign_id: str
    platform: str
    post_type: str
    platform_post_id: str | None = None
    url: str | None = None
    title: str | None = None
    body_preview: str | None = None
    target_community: str | None = None
    posted_at: datetime | None = None
    tags: str | None = None
    is_api_tracked: int = 0
    created_at: datetime | None = None

    model_config = {"from_attributes": True}


@router.get("/api/campaigns/{campaign_id}/posts", response_model=list[PostResponse])
async def list_posts(campaign_id: str, db: AsyncSession = Depends(get_db)):
    # Verify campaign exists
    camp_result = await db.execute(select(Campaign).where(Campaign.id == campaign_id))
    if not camp_result.scalar_one_or_none():
        raise HTTPException(status_code=404, detail="Campaign not found")

    result = await db.execute(
        select(Post)
        .where(Post.campaign_id == campaign_id)
        .order_by(Post.created_at.desc())
    )
    return result.scalars().all()


@router.post(
    "/api/campaigns/{campaign_id}/posts",
    response_model=PostResponse,
    status_code=201,
)
async def create_post(
    campaign_id: str, data: PostCreate, db: AsyncSession = Depends(get_db)
):
    camp_result = await db.execute(select(Campaign).where(Campaign.id == campaign_id))
    if not camp_result.scalar_one_or_none():
        raise HTTPException(status_code=404, detail="Campaign not found")

    post = Post(id=str(uuid4()), campaign_id=campaign_id, **data.model_dump())
    db.add(post)
    await db.commit()
    await db.refresh(post)
    return post


@router.put("/api/posts/{post_id}", response_model=PostResponse)
async def update_post(
    post_id: str, data: PostUpdate, db: AsyncSession = Depends(get_db)
):
    result = await db.execute(select(Post).where(Post.id == post_id))
    post = result.scalar_one_or_none()
    if not post:
        raise HTTPException(status_code=404, detail="Post not found")
    for key, value in data.model_dump(exclude_unset=True).items():
        setattr(post, key, value)
    await db.commit()
    await db.refresh(post)
    return post


@router.delete("/api/posts/{post_id}", status_code=204)
async def delete_post(post_id: str, db: AsyncSession = Depends(get_db)):
    result = await db.execute(select(Post).where(Post.id == post_id))
    post = result.scalar_one_or_none()
    if not post:
        raise HTTPException(status_code=404, detail="Post not found")
    await db.delete(post)
    await db.commit()
