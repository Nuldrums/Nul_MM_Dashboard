"""Reddit API connector using asyncpraw."""

import logging
import re
from datetime import date

import asyncpraw

from backend.connectors.base import PlatformConnector, MetricResult

logger = logging.getLogger(__name__)


class RedditConnector(PlatformConnector):
    platform = "reddit"

    def __init__(
        self,
        client_id: str = "",
        client_secret: str = "",
        username: str = "",
        password: str = "",
    ):
        self.client_id = client_id
        self.client_secret = client_secret
        self.username = username
        self.password = password

    def _has_credentials(self) -> bool:
        return bool(self.client_id and self.client_secret)

    def _create_reddit(self) -> asyncpraw.Reddit:
        kwargs = {
            "client_id": self.client_id,
            "client_secret": self.client_secret,
            "user_agent": "TrikeriMarketingEngine/0.1",
        }
        if self.username and self.password:
            kwargs["username"] = self.username
            kwargs["password"] = self.password
        return asyncpraw.Reddit(**kwargs)

    async def validate_credentials(self) -> bool:
        """Test Reddit API credentials via asyncpraw."""
        if not self._has_credentials():
            return False
        try:
            reddit = self._create_reddit()
            # Attempt a lightweight API call
            user = await reddit.user.me()
            await reddit.close()
            return True
        except Exception as e:
            logger.warning("Reddit credential validation failed: %s", e)
            try:
                await reddit.close()
            except Exception:
                pass
            return False

    async def fetch_post_metrics(self, platform_post_id: str, **kwargs) -> MetricResult:
        """Fetch metrics for a Reddit submission.

        Mapping:
          likes = score (net upvotes)
          comments = num_comments
          shares = num_crossposts
          views = view_count (if available, usually None for non-OC)
        """
        if not self._has_credentials():
            raise ValueError(
                "Reddit API credentials not configured. "
                "Set TRIKERI_REDDIT_CLIENT_ID and TRIKERI_REDDIT_CLIENT_SECRET."
            )

        reddit = self._create_reddit()
        try:
            submission = await reddit.submission(id=platform_post_id)
            await submission.load()

            return MetricResult(
                views=getattr(submission, "view_count", 0) or 0,
                likes=submission.score,
                comments=submission.num_comments,
                shares=getattr(submission, "num_crossposts", 0) or 0,
                fetched_via="api",
                snapshot_date=date.today(),
                custom_metrics={
                    "upvote_ratio": submission.upvote_ratio,
                    "subreddit": str(submission.subreddit),
                    "is_original_content": submission.is_original_content,
                    "over_18": submission.over_18,
                },
            )
        finally:
            await reddit.close()

    async def resolve_post_id(self, url: str) -> str | None:
        """Extract Reddit post ID from URL.

        Handles: reddit.com/r/subreddit/comments/ABC123/...
        Returns: ABC123
        """
        match = re.search(r"/comments/([a-zA-Z0-9]+)", url)
        return match.group(1) if match else None

    def is_api_trackable(self) -> bool:
        return True
