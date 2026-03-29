"""X/Twitter API v2 connector using httpx."""

import logging
import re
from datetime import date

import httpx

from backend.connectors.base import PlatformConnector, MetricResult

logger = logging.getLogger(__name__)

X_API_BASE = "https://api.x.com/2"


class TwitterConnector(PlatformConnector):
    platform = "x"

    def __init__(self, bearer_token: str = ""):
        self.bearer_token = bearer_token

    def _headers(self) -> dict:
        return {
            "Authorization": f"Bearer {self.bearer_token}",
            "Content-Type": "application/json",
        }

    async def validate_credentials(self) -> bool:
        """Test X API v2 bearer token."""
        if not self.bearer_token:
            return False
        try:
            async with httpx.AsyncClient() as client:
                # Use a lightweight endpoint to test the token
                resp = await client.get(
                    f"{X_API_BASE}/users/me",
                    headers=self._headers(),
                    timeout=10,
                )
                # Bearer tokens with app-only auth won't have /users/me,
                # but a 401 means bad token, 403 means valid token but wrong scope
                return resp.status_code != 401
        except Exception as e:
            logger.warning("Twitter credential validation failed: %s", e)
            return False

    async def fetch_post_metrics(self, platform_post_id: str, **kwargs) -> MetricResult:
        """Fetch metrics for a tweet using X API v2.

        Mapping:
          views = impression_count
          impressions = impression_count
          likes = like_count
          comments = reply_count
          shares = retweet_count + quote_count
          saves = bookmark_count
        """
        if not self.bearer_token:
            raise ValueError(
                "X/Twitter bearer token not configured. "
                "Set TRIKERI_TWITTER_BEARER_TOKEN in your .env file."
            )

        async with httpx.AsyncClient() as client:
            resp = await client.get(
                f"{X_API_BASE}/tweets/{platform_post_id}",
                headers=self._headers(),
                params={
                    "tweet.fields": "public_metrics,non_public_metrics,organic_metrics",
                },
                timeout=15,
            )

            if resp.status_code == 401:
                raise ConnectionError("Invalid X API bearer token")
            if resp.status_code == 404:
                raise ValueError(f"Tweet {platform_post_id} not found")
            if resp.status_code != 200:
                raise ConnectionError(
                    f"X API returned status {resp.status_code}: {resp.text}"
                )

            data = resp.json().get("data", {})
            public = data.get("public_metrics", {})

            impression_count = public.get("impression_count", 0)
            like_count = public.get("like_count", 0)
            reply_count = public.get("reply_count", 0)
            retweet_count = public.get("retweet_count", 0)
            quote_count = public.get("quote_count", 0)
            bookmark_count = public.get("bookmark_count", 0)

            return MetricResult(
                views=impression_count,
                impressions=impression_count,
                likes=like_count,
                comments=reply_count,
                shares=retweet_count + quote_count,
                saves=bookmark_count,
                fetched_via="api",
                snapshot_date=date.today(),
                custom_metrics={
                    "retweet_count": retweet_count,
                    "quote_count": quote_count,
                    "bookmark_count": bookmark_count,
                },
            )

    async def resolve_post_id(self, url: str) -> str | None:
        """Extract tweet ID from URL.

        Handles: x.com/username/status/1234567890
                 twitter.com/username/status/1234567890
        """
        match = re.search(r"/status/(\d+)", url)
        return match.group(1) if match else None

    def is_api_trackable(self) -> bool:
        return True
