"""Configuration management and system endpoints."""

import json
from datetime import datetime, timedelta
from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel
from sqlalchemy import select, text
from sqlalchemy.ext.asyncio import AsyncSession

from backend.database.connection import get_db
from backend.database.models import PlatformConfig, SystemState

router = APIRouter(tags=["settings"])


class PlatformConfigUpdate(BaseModel):
    credentials: dict | None = None
    is_enabled: bool = False
    config: dict | None = None


class GeneralSettingsUpdate(BaseModel):
    data_dir: str | None = None
    auto_fetch_interval_hours: int | None = None
    auto_analysis_interval_hours: int | None = None


SUPPORTED_PLATFORMS = [
    "reddit", "youtube", "twitter", "discord", "hackernews",
    "producthunt", "tiktok", "instagram", "linkedin", "other",
]


@router.get("/api/settings")
async def get_settings(db: AsyncSession = Depends(get_db)):
    """All settings with API keys redacted."""
    platforms_result = await db.execute(select(PlatformConfig))
    platforms = {}
    for pc in platforms_result.scalars().all():
        creds = json.loads(pc.credentials) if pc.credentials else {}
        # Redact credential values
        redacted_creds = {k: "***" if v else "" for k, v in creds.items()}
        platforms[pc.platform] = {
            "is_enabled": bool(pc.is_enabled),
            "credentials_set": {k: bool(v) for k, v in creds.items()},
            "credentials_redacted": redacted_creds,
            "last_fetched_at": pc.last_fetched_at.isoformat() if pc.last_fetched_at else None,
            "config": json.loads(pc.config) if pc.config else {},
        }

    # Get general settings from system_state
    general_result = await db.execute(select(SystemState))
    general = {s.key: s.value for s in general_result.scalars().all()}

    return {
        "platforms": platforms,
        "general": general,
        "supported_platforms": SUPPORTED_PLATFORMS,
    }


@router.put("/api/settings/platform/{platform_name}")
async def update_platform_config(
    platform_name: str,
    data: PlatformConfigUpdate,
    db: AsyncSession = Depends(get_db),
):
    """Update platform credentials."""
    if platform_name not in SUPPORTED_PLATFORMS:
        raise HTTPException(status_code=400, detail=f"Unsupported platform: {platform_name}")

    result = await db.execute(
        select(PlatformConfig).where(PlatformConfig.platform == platform_name)
    )
    pc = result.scalar_one_or_none()

    if pc:
        if data.credentials is not None:
            pc.credentials = json.dumps(data.credentials)
        pc.is_enabled = int(data.is_enabled)
        if data.config is not None:
            pc.config = json.dumps(data.config)
    else:
        pc = PlatformConfig(
            platform=platform_name,
            credentials=json.dumps(data.credentials or {}),
            is_enabled=int(data.is_enabled),
            config=json.dumps(data.config or {}),
        )
        db.add(pc)

    await db.commit()
    return {"message": f"Platform config updated for {platform_name}"}


@router.put("/api/settings/general")
async def update_general_settings(
    data: GeneralSettingsUpdate, db: AsyncSession = Depends(get_db)
):
    """Update general settings stored in system_state."""
    for key, value in data.model_dump(exclude_unset=True).items():
        if value is not None:
            result = await db.execute(
                select(SystemState).where(SystemState.key == key)
            )
            state = result.scalar_one_or_none()
            if state:
                state.value = str(value)
                state.updated_at = datetime.utcnow()
            else:
                db.add(SystemState(key=key, value=str(value), updated_at=datetime.utcnow()))
    await db.commit()
    return {"message": "General settings updated"}


@router.get("/api/settings/health")
async def platform_health(db: AsyncSession = Depends(get_db)):
    """Check which platform APIs are connected and healthy."""
    result = await db.execute(select(PlatformConfig))
    health = {}
    for pc in result.scalars().all():
        creds = json.loads(pc.credentials) if pc.credentials else {}
        has_creds = any(bool(v) for v in creds.values())
        health[pc.platform] = {
            "enabled": bool(pc.is_enabled),
            "credentials_configured": has_creds,
            "last_fetched_at": pc.last_fetched_at.isoformat() if pc.last_fetched_at else None,
            "status": "ready" if has_creds and pc.is_enabled else "not_configured",
        }

    # HN is always available (no auth needed)
    if "hackernews" not in health:
        health["hackernews"] = {
            "enabled": True,
            "credentials_configured": True,
            "last_fetched_at": None,
            "status": "ready",
        }

    return health


@router.get("/api/system/startup-check")
async def startup_check(db: AsyncSession = Depends(get_db)):
    """Returns what needs updating (metrics, AI analysis)."""
    now = datetime.utcnow()

    # Check metric staleness
    fetch_result = await db.execute(
        select(SystemState).where(SystemState.key == "last_metric_fetch")
    )
    fetch_state = fetch_result.scalar_one_or_none()
    metrics_last_run = fetch_state.value if fetch_state else None
    metrics_stale = True
    if metrics_last_run:
        try:
            last_dt = datetime.fromisoformat(metrics_last_run)
            metrics_stale = (now - last_dt) > timedelta(hours=6)
        except (ValueError, TypeError):
            pass

    # Check analysis staleness
    analysis_result = await db.execute(
        select(SystemState).where(SystemState.key == "last_ai_analysis")
    )
    analysis_state = analysis_result.scalar_one_or_none()
    analysis_last_run = analysis_state.value if analysis_state else None
    analysis_stale = True
    if analysis_last_run:
        try:
            last_dt = datetime.fromisoformat(analysis_last_run)
            analysis_stale = (now - last_dt) > timedelta(hours=24)
        except (ValueError, TypeError):
            pass

    return {
        "metrics_stale": metrics_stale,
        "metrics_last_run": metrics_last_run,
        "analysis_stale": analysis_stale,
        "analysis_last_run": analysis_last_run,
        "server_time": now.isoformat(),
    }


@router.post("/api/system/export/{campaign_id}")
async def export_campaign(campaign_id: str, db: AsyncSession = Depends(get_db)):
    """Export campaign data (placeholder)."""
    return {
        "message": "Export functionality coming soon",
        "campaign_id": campaign_id,
        "formats_planned": ["json", "csv"],
    }
