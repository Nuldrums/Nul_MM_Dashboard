"""24-hour analysis trigger logic.

While the app is running, periodically checks if it's time
to run the daily AI analysis pipeline.
"""

import asyncio
import logging
from datetime import datetime, timedelta

from sqlalchemy import select

from backend.database.connection import AsyncSessionLocal
from backend.database.models import SystemState

logger = logging.getLogger(__name__)


class AnalysisScheduler:
    """Background scheduler that triggers AI analysis every 24 hours.

    Checks hourly whether 24h has passed since the last analysis.
    This handles the case where the app is left open for days.
    """

    def __init__(self):
        self._task: asyncio.Task | None = None
        self._running = False

    async def start(self):
        """Start the background scheduler loop."""
        if self._running:
            return
        self._running = True
        self._task = asyncio.create_task(self._loop())
        logger.info("Analysis scheduler started")

    async def _loop(self):
        """Check every hour if analysis is needed."""
        while self._running:
            await asyncio.sleep(3600)  # Check every hour
            try:
                await self._check_and_trigger()
            except Exception as e:
                logger.error("Scheduler check failed: %s", e)

    async def _check_and_trigger(self):
        """Check if 24h has passed since last analysis and trigger if needed."""
        async with AsyncSessionLocal() as session:
            result = await session.execute(
                select(SystemState).where(SystemState.key == "last_ai_analysis")
            )
            state = result.scalar_one_or_none()

            now = datetime.utcnow()
            should_run = True

            if state and state.value:
                try:
                    last_run = datetime.fromisoformat(state.value)
                    should_run = (now - last_run) > timedelta(hours=24)
                except (ValueError, TypeError):
                    pass

            if should_run:
                logger.info("24h threshold reached, triggering daily pipeline")
                try:
                    from backend.services.daily_pipeline import DailyPipeline

                    pipeline = DailyPipeline()
                    report = await pipeline.run_full(session)

                    if report.success:
                        logger.info(
                            "Scheduled pipeline completed successfully: %d steps",
                            len(report.steps),
                        )
                    else:
                        logger.error(
                            "Scheduled pipeline failed: %s", report.error
                        )
                except Exception as e:
                    logger.error("Failed to run daily pipeline: %s", e)

                    # Even on failure, update the timestamp to avoid rapid retries
                    if state:
                        state.value = now.isoformat()
                        state.updated_at = now
                    else:
                        session.add(
                            SystemState(
                                key="last_ai_analysis",
                                value=now.isoformat(),
                                updated_at=now,
                            )
                        )
                    await session.commit()

    async def stop(self):
        """Stop the background scheduler."""
        self._running = False
        if self._task:
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass
            self._task = None
        logger.info("Analysis scheduler stopped")
