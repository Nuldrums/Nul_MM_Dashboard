"""AI analysis endpoints."""

import asyncio
import json
import logging
from datetime import datetime

from fastapi import APIRouter, Depends, HTTPException, BackgroundTasks
from sqlalchemy import select, func
from sqlalchemy.ext.asyncio import AsyncSession

from backend.config import settings
from backend.database.connection import get_db, AsyncSessionLocal
from backend.database.models import AIAnalysis, Campaign, SystemState
from backend.ai.embedder import MarketingKnowledgeBase
from backend.services.daily_pipeline import DailyPipeline

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api/ai", tags=["ai"])

# In-memory state for analysis trigger
_analysis_state = {"running": False, "last_started": None, "last_completed": None, "error": None}

# Shared knowledge base instance
_knowledge_base = MarketingKnowledgeBase()


async def _run_pipeline_background():
    """Run the daily pipeline in a background task."""
    _analysis_state["running"] = True
    _analysis_state["last_started"] = datetime.utcnow().isoformat()
    _analysis_state["error"] = None

    try:
        pipeline = DailyPipeline()
        async with AsyncSessionLocal() as session:
            report = await pipeline.run_full(session)

        if report.success:
            _analysis_state["last_completed"] = datetime.utcnow().isoformat()
            logger.info("Background pipeline completed successfully")
        else:
            _analysis_state["error"] = report.error
            logger.error("Background pipeline failed: %s", report.error)
    except Exception as e:
        _analysis_state["error"] = str(e)
        logger.error("Background pipeline exception: %s", e)
    finally:
        _analysis_state["running"] = False


@router.get("/latest")
async def latest_analyses(db: AsyncSession = Depends(get_db)):
    """Most recent analysis for each active campaign."""
    campaigns = await db.execute(
        select(Campaign).where(Campaign.status == "active")
    )
    results = []
    for campaign in campaigns.scalars().all():
        analysis = await db.execute(
            select(AIAnalysis)
            .where(AIAnalysis.campaign_id == campaign.id)
            .order_by(AIAnalysis.analyzed_at.desc())
            .limit(1)
        )
        a = analysis.scalar_one_or_none()
        if a:
            results.append({
                "id": a.id,
                "campaign_id": a.campaign_id,
                "campaign_name": campaign.name,
                "analysis_type": a.analysis_type,
                "summary": a.summary,
                "top_performers": json.loads(a.top_performers) if a.top_performers else [],
                "underperformers": json.loads(a.underperformers) if a.underperformers else [],
                "patterns": json.loads(a.patterns) if a.patterns else [],
                "recommendations": json.loads(a.recommendations) if a.recommendations else [],
                "model_used": a.model_used,
                "tokens_used": a.tokens_used,
                "analyzed_at": a.analyzed_at.isoformat() if a.analyzed_at else None,
            })
    return results


@router.get("/campaign/{campaign_id}")
async def campaign_analyses(
    campaign_id: str, db: AsyncSession = Depends(get_db)
):
    """All analyses for a campaign."""
    result = await db.execute(select(Campaign).where(Campaign.id == campaign_id))
    if not result.scalar_one_or_none():
        raise HTTPException(status_code=404, detail="Campaign not found")

    analyses = await db.execute(
        select(AIAnalysis)
        .where(AIAnalysis.campaign_id == campaign_id)
        .order_by(AIAnalysis.analyzed_at.desc())
    )
    return [
        {
            "id": a.id,
            "campaign_id": a.campaign_id,
            "analysis_type": a.analysis_type,
            "summary": a.summary,
            "top_performers": json.loads(a.top_performers) if a.top_performers else [],
            "underperformers": json.loads(a.underperformers) if a.underperformers else [],
            "patterns": json.loads(a.patterns) if a.patterns else [],
            "recommendations": json.loads(a.recommendations) if a.recommendations else [],
            "model_used": a.model_used,
            "tokens_used": a.tokens_used,
            "analyzed_at": a.analyzed_at.isoformat() if a.analyzed_at else None,
        }
        for a in analyses.scalars().all()
    ]


@router.post("/trigger")
async def trigger_analysis(background_tasks: BackgroundTasks):
    """Manually trigger AI analysis pipeline."""
    if _analysis_state["running"]:
        return {"message": "Analysis already in progress", "status": "running"}

    if not settings.anthropic_api_key:
        return {
            "message": "No Anthropic API key configured. Set TRIKERI_ANTHROPIC_API_KEY in .env.",
            "status": "error",
        }

    background_tasks.add_task(_run_pipeline_background)
    return {"message": "Analysis pipeline started", "status": "started"}


@router.get("/status")
async def analysis_status(db: AsyncSession = Depends(get_db)):
    """Last run time, next scheduled, running state."""
    last_analysis = await db.execute(
        select(SystemState).where(SystemState.key == "last_ai_analysis")
    )
    last = last_analysis.scalar_one_or_none()

    return {
        "running": _analysis_state["running"],
        "last_run": last.value if last else None,
        "last_started": _analysis_state["last_started"],
        "last_completed": _analysis_state["last_completed"],
        "error": _analysis_state["error"],
        "api_key_configured": bool(settings.anthropic_api_key),
        "next_scheduled": None,  # Will be computed by scheduler
    }


@router.get("/recommendations")
async def recommendations(db: AsyncSession = Depends(get_db)):
    """Cross-campaign strategic recommendations from the latest cross-campaign analysis."""
    result = await db.execute(
        select(AIAnalysis)
        .where(AIAnalysis.analysis_type == "cross_campaign")
        .order_by(AIAnalysis.analyzed_at.desc())
        .limit(1)
    )
    analysis = result.scalar_one_or_none()
    if not analysis:
        return {"recommendations": [], "message": "No cross-campaign analysis available yet"}

    return {
        "recommendations": json.loads(analysis.recommendations) if analysis.recommendations else [],
        "patterns": json.loads(analysis.patterns) if analysis.patterns else [],
        "analyzed_at": analysis.analyzed_at.isoformat() if analysis.analyzed_at else None,
    }


@router.get("/knowledge-base/query")
async def knowledge_base_query(q: str = ""):
    """Semantic search the vector DB."""
    if not q:
        return {"query": q, "results": [], "message": "Provide a query parameter 'q'."}

    try:
        await _knowledge_base.initialize()
        results = await _knowledge_base.query_similar(query_text=q, n=10)
        return {
            "query": q,
            "results": results,
            "count": len(results),
        }
    except RuntimeError:
        return {
            "query": q,
            "results": [],
            "message": "Knowledge base not yet initialized. Run an analysis first.",
        }
    except Exception as e:
        logger.error("Knowledge base query error: %s", e)
        return {
            "query": q,
            "results": [],
            "message": f"Knowledge base query failed: {e}",
        }


@router.get("/knowledge-base/stats")
async def knowledge_base_stats():
    """Vector DB size, coverage, last update."""
    try:
        await _knowledge_base.initialize()
        stats = await _knowledge_base.get_stats()
        return stats
    except Exception as e:
        return {
            "initialized": False,
            "total_documents": 0,
            "coverage": {
                "campaigns_embedded": 0,
                "posts_embedded": 0,
                "patterns_stored": 0,
            },
            "message": f"Knowledge base not available: {e}",
        }
