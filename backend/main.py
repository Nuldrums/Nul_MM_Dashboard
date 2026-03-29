"""FastAPI application for the Trikeri Marketing Engine backend."""

from contextlib import asynccontextmanager
from datetime import datetime, timedelta

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from sqlalchemy import select

from backend.config import settings
from backend.database.connection import init_db, AsyncSessionLocal
from backend.database.models import SystemState
from backend.database.migrations import run_migrations
from backend.ai.scheduler import AnalysisScheduler

from backend.routers import (
    profiles,
    products,
    campaigns,
    posts,
    metrics,
    analytics,
    ai_analysis,
    settings as settings_router,
)

scheduler = AnalysisScheduler()


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Startup and shutdown hooks."""
    # --- Startup ---
    await init_db()

    # Run migrations
    async with AsyncSessionLocal() as session:
        await run_migrations(session)

    # Staleness check
    await _startup_staleness_check()

    # Start background scheduler
    await scheduler.start()

    yield

    # --- Shutdown ---
    await scheduler.stop()


async def _startup_staleness_check():
    """Check system_state for staleness on startup.

    If metrics are >6h stale, flag for fetch.
    If AI analysis is >24h stale, flag for analysis.
    """
    async with AsyncSessionLocal() as session:
        now = datetime.utcnow()

        # Check metric fetch staleness
        fetch_result = await session.execute(
            select(SystemState).where(SystemState.key == "last_metric_fetch")
        )
        fetch_state = fetch_result.scalar_one_or_none()
        metrics_stale = True
        if fetch_state and fetch_state.value:
            try:
                last_fetch = datetime.fromisoformat(fetch_state.value)
                metrics_stale = (now - last_fetch) > timedelta(hours=6)
            except (ValueError, TypeError):
                pass

        # Check AI analysis staleness
        analysis_result = await session.execute(
            select(SystemState).where(SystemState.key == "last_ai_analysis")
        )
        analysis_state = analysis_result.scalar_one_or_none()
        analysis_stale = True
        if analysis_state and analysis_state.value:
            try:
                last_analysis = datetime.fromisoformat(analysis_state.value)
                analysis_stale = (now - last_analysis) > timedelta(hours=24)
            except (ValueError, TypeError):
                pass

        if metrics_stale:
            # In production: background_tasks.add_task(metric_collector.collect_all)
            # For now, just log it
            print(f"[startup] Metrics are stale (last: {fetch_state.value if fetch_state else 'never'})")

        if analysis_stale:
            print(f"[startup] AI analysis is stale (last: {analysis_state.value if analysis_state else 'never'})")


app = FastAPI(
    title="Trikeri Marketing Engine",
    description="Backend API for the Trikeri multi-platform marketing dashboard",
    version="0.1.0",
    lifespan=lifespan,
)

# CORS middleware — allow all origins for development
app.add_middleware(
    CORSMiddleware,
    allow_origins=settings.cors_origins,
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Include all routers
app.include_router(profiles.router)
app.include_router(products.router)
app.include_router(campaigns.router)
app.include_router(posts.router)
app.include_router(metrics.router)
app.include_router(analytics.router)
app.include_router(ai_analysis.router)
app.include_router(settings_router.router)


@app.get("/api/health")
async def health_check():
    """Basic health check endpoint."""
    return {
        "status": "ok",
        "service": "trikeri-marketing-engine",
        "version": "0.1.0",
    }


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        "backend.main:app",
        host="0.0.0.0",
        port=settings.api_port,
        reload=True,
    )
