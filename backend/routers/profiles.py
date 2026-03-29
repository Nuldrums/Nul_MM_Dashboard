"""CRUD endpoints for profiles."""

from uuid import uuid4
from datetime import datetime
from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel
from sqlalchemy import select, func
from sqlalchemy.ext.asyncio import AsyncSession

from backend.database.connection import get_db
from backend.database.models import Profile, Campaign

router = APIRouter(prefix="/api/profiles", tags=["profiles"])


class ProfileCreate(BaseModel):
    name: str
    description: str | None = None
    avatar_color: str = "#E8845C"


class ProfileUpdate(BaseModel):
    name: str | None = None
    description: str | None = None
    avatar_color: str | None = None


class ProfileResponse(BaseModel):
    id: str
    name: str
    description: str | None = None
    avatar_color: str | None = None
    created_at: datetime | None = None

    model_config = {"from_attributes": True}


@router.get("", response_model=list[ProfileResponse])
async def list_profiles(db: AsyncSession = Depends(get_db)):
    result = await db.execute(select(Profile).order_by(Profile.created_at.asc()))
    return result.scalars().all()


@router.post("", response_model=ProfileResponse, status_code=201)
async def create_profile(data: ProfileCreate, db: AsyncSession = Depends(get_db)):
    # Check for duplicate name
    existing = await db.execute(
        select(Profile).where(Profile.name == data.name)
    )
    if existing.scalar_one_or_none():
        raise HTTPException(status_code=409, detail="Profile with this name already exists")

    profile = Profile(id=str(uuid4()), **data.model_dump())
    db.add(profile)
    await db.commit()
    await db.refresh(profile)
    return profile


@router.put("/{profile_id}", response_model=ProfileResponse)
async def update_profile(
    profile_id: str, data: ProfileUpdate, db: AsyncSession = Depends(get_db)
):
    result = await db.execute(select(Profile).where(Profile.id == profile_id))
    profile = result.scalar_one_or_none()
    if not profile:
        raise HTTPException(status_code=404, detail="Profile not found")

    # Check name uniqueness if changing name
    if data.name and data.name != profile.name:
        existing = await db.execute(
            select(Profile).where(Profile.name == data.name)
        )
        if existing.scalar_one_or_none():
            raise HTTPException(status_code=409, detail="Profile with this name already exists")

    for key, value in data.model_dump(exclude_unset=True).items():
        setattr(profile, key, value)
    await db.commit()
    await db.refresh(profile)
    return profile


@router.delete("/{profile_id}", status_code=200)
async def delete_profile(profile_id: str, db: AsyncSession = Depends(get_db)):
    result = await db.execute(select(Profile).where(Profile.id == profile_id))
    profile = result.scalar_one_or_none()
    if not profile:
        raise HTTPException(status_code=404, detail="Profile not found")

    # Check for attached campaigns
    campaign_count = (await db.execute(
        select(func.count(Campaign.id)).where(Campaign.profile_id == profile_id)
    )).scalar() or 0

    if campaign_count > 0:
        raise HTTPException(
            status_code=409,
            detail=f"Cannot delete profile with {campaign_count} attached campaign(s). Reassign or delete them first."
        )

    await db.delete(profile)
    await db.commit()
    return {"message": "Profile deleted", "id": profile_id}
