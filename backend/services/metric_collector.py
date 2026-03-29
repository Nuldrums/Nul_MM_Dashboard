"""Orchestrates metric collection across all platform connectors."""

import json
import logging
from dataclasses import dataclass, field
from datetime import date, datetime

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from backend.config import settings
from backend.database.models import Post, MetricSnapshot, SystemState
from backend.connectors.base import PlatformConnector, MetricResult
from backend.connectors.hackernews import HackerNewsConnector
from backend.connectors.manual import ManualConnector
from backend.connectors.reddit import RedditConnector
from backend.connectors.youtube import YouTubeConnector
from backend.connectors.twitter import TwitterConnector
from backend.connectors.discord import DiscordConnector

logger = logging.getLogger(__name__)


@dataclass
class CollectionReport:
    """Summary of a metric collection run."""
    fetched: int = 0
    failed: int = 0
    skipped: int = 0
    manual_needed: int = 0
    errors: list[str] = field(default_factory=list)
    started_at: datetime | None = None
    completed_at: datetime | None = None


class MetricCollector:
    """Runs metric collection for all tracked posts.

    Iterates all posts with is_api_tracked=1, groups by platform,
    calls the appropriate connector, and writes MetricSnapshot rows.
    """

    def __init__(self):
        self._connectors: dict[str, PlatformConnector] = {}
        self._init_connectors()

    def _init_connectors(self):
        """Initialize all connectors with configured credentials."""
        # HackerNews - no credentials needed
        self._connectors["hackernews"] = HackerNewsConnector()

        # Manual fallback
        self._connectors["manual"] = ManualConnector()

        # Discord - manual only for MVP
        self._connectors["discord"] = DiscordConnector(
            bot_token=settings.discord_bot_token,
        )

        # Reddit - only if credentials configured
        if settings.reddit_client_id and settings.reddit_client_secret:
            self._connectors["reddit"] = RedditConnector(
                client_id=settings.reddit_client_id,
                client_secret=settings.reddit_client_secret,
                username=settings.reddit_username,
                password=settings.reddit_password,
            )
            logger.info("Reddit connector loaded with credentials")
        else:
            # Register a connector that will gracefully skip
            self._connectors["reddit"] = RedditConnector()
            logger.info("Reddit connector loaded without credentials (will skip API calls)")

        # YouTube - only if API key configured
        if settings.youtube_api_key:
            self._connectors["youtube"] = YouTubeConnector(
                api_key=settings.youtube_api_key,
            )
            logger.info("YouTube connector loaded with API key")
        else:
            self._connectors["youtube"] = YouTubeConnector()
            logger.info("YouTube connector loaded without API key (will skip API calls)")

        # Twitter/X - only if bearer token configured
        if settings.twitter_bearer_token:
            self._connectors["x"] = TwitterConnector(
                bearer_token=settings.twitter_bearer_token,
            )
            # Also register under "twitter" alias
            self._connectors["twitter"] = self._connectors["x"]
            logger.info("Twitter/X connector loaded with bearer token")
        else:
            self._connectors["x"] = TwitterConnector()
            self._connectors["twitter"] = self._connectors["x"]
            logger.info("Twitter/X connector loaded without bearer token (will skip API calls)")

    def register_connector(self, platform: str, connector: PlatformConnector):
        """Register a platform connector for use in collection."""
        self._connectors[platform] = connector

    async def collect_all(self, session: AsyncSession) -> CollectionReport:
        """Collect metrics for all API-tracked posts.

        1. Load all posts where is_api_tracked = 1
        2. Group by platform
        3. For each platform, load the connector
        4. Fetch metrics for each post
        5. Write MetricSnapshot rows (UPSERT on post_id + snapshot_date)
        6. Return summary report
        """
        report = CollectionReport(started_at=datetime.utcnow())

        result = await session.execute(
            select(Post).where(Post.is_api_tracked == 1)
        )
        posts = result.scalars().all()

        # Group by platform
        by_platform: dict[str, list] = {}
        for post in posts:
            by_platform.setdefault(post.platform, []).append(post)

        for platform, platform_posts in by_platform.items():
            connector = self._connectors.get(platform)
            if not connector or not connector.is_api_trackable():
                report.manual_needed += len(platform_posts)
                continue

            for post in platform_posts:
                if not post.platform_post_id:
                    report.skipped += 1
                    continue
                try:
                    metrics = await connector.fetch_post_metrics(post.platform_post_id)
                    await self._upsert_snapshot(session, post.id, metrics)
                    report.fetched += 1
                except (NotImplementedError, ValueError) as e:
                    report.skipped += 1
                    logger.debug("Skipped %s: %s", post.id, e)
                except Exception as e:
                    report.failed += 1
                    report.errors.append(f"{post.id}: {str(e)}")
                    logger.warning("Failed to fetch metrics for %s: %s", post.id, e)

        # Update system state
        now = datetime.utcnow()
        state_result = await session.execute(
            select(SystemState).where(SystemState.key == "last_metric_fetch")
        )
        state = state_result.scalar_one_or_none()
        if state:
            state.value = now.isoformat()
            state.updated_at = now
        else:
            session.add(SystemState(
                key="last_metric_fetch",
                value=now.isoformat(),
                updated_at=now,
            ))

        await session.commit()
        report.completed_at = datetime.utcnow()

        logger.info(
            "Collection complete: fetched=%d, failed=%d, skipped=%d, manual=%d",
            report.fetched, report.failed, report.skipped, report.manual_needed,
        )
        return report

    async def collect_campaign(
        self, session: AsyncSession, campaign_id: str
    ) -> CollectionReport:
        """Collect metrics scoped to one campaign's posts."""
        report = CollectionReport(started_at=datetime.utcnow())

        result = await session.execute(
            select(Post).where(
                Post.campaign_id == campaign_id,
                Post.is_api_tracked == 1,
            )
        )
        posts = result.scalars().all()

        for post in posts:
            connector = self._connectors.get(post.platform)
            if not connector or not connector.is_api_trackable():
                report.manual_needed += 1
                continue
            if not post.platform_post_id:
                report.skipped += 1
                continue
            try:
                metrics = await connector.fetch_post_metrics(post.platform_post_id)
                await self._upsert_snapshot(session, post.id, metrics)
                report.fetched += 1
            except (NotImplementedError, ValueError) as e:
                report.skipped += 1
            except Exception as e:
                report.failed += 1
                report.errors.append(f"{post.id}: {str(e)}")

        await session.commit()
        report.completed_at = datetime.utcnow()
        return report

    async def collect_post(
        self, session: AsyncSession, post_id: str
    ) -> MetricResult | None:
        """Fetch metrics for a single post on demand."""
        result = await session.execute(select(Post).where(Post.id == post_id))
        post = result.scalar_one_or_none()
        if not post or not post.platform_post_id:
            return None

        connector = self._connectors.get(post.platform)
        if not connector or not connector.is_api_trackable():
            return None

        metrics = await connector.fetch_post_metrics(post.platform_post_id)
        await self._upsert_snapshot(session, post.id, metrics)
        await session.commit()
        return metrics

    async def _upsert_snapshot(
        self, session: AsyncSession, post_id: str, metrics: MetricResult
    ):
        """Insert or update a metric snapshot for today."""
        today = metrics.snapshot_date or date.today()

        # Check for existing snapshot
        result = await session.execute(
            select(MetricSnapshot).where(
                MetricSnapshot.post_id == post_id,
                MetricSnapshot.snapshot_date == today,
            )
        )
        existing = result.scalar_one_or_none()

        if existing:
            existing.views = metrics.views
            existing.impressions = metrics.impressions
            existing.likes = metrics.likes
            existing.dislikes = metrics.dislikes
            existing.comments = metrics.comments
            existing.shares = metrics.shares
            existing.saves = metrics.saves
            existing.clicks = metrics.clicks
            existing.watch_time_seconds = metrics.watch_time_seconds
            existing.followers_gained = metrics.followers_gained
            existing.fetched_via = metrics.fetched_via
            if metrics.custom_metrics:
                existing.custom_metrics = json.dumps(metrics.custom_metrics)
        else:
            snapshot = MetricSnapshot(
                post_id=post_id,
                snapshot_date=today,
                views=metrics.views,
                impressions=metrics.impressions,
                likes=metrics.likes,
                dislikes=metrics.dislikes,
                comments=metrics.comments,
                shares=metrics.shares,
                saves=metrics.saves,
                clicks=metrics.clicks,
                watch_time_seconds=metrics.watch_time_seconds,
                followers_gained=metrics.followers_gained,
                custom_metrics=json.dumps(metrics.custom_metrics) if metrics.custom_metrics else None,
                fetched_via=metrics.fetched_via,
            )
            session.add(snapshot)
