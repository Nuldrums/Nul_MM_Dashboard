"""Hacker News connector using the open Firebase API (no auth required)."""

import re
from datetime import date

import httpx

from backend.connectors.base import PlatformConnector, MetricResult

HN_API_BASE = "https://hacker-news.firebaseio.com/v0"


class HackerNewsConnector(PlatformConnector):
    platform = "hackernews"

    async def validate_credentials(self) -> bool:
        """HN API is open, no credentials needed. Just verify API is reachable."""
        try:
            async with httpx.AsyncClient() as client:
                resp = await client.get(f"{HN_API_BASE}/topstories.json", timeout=10)
                return resp.status_code == 200
        except Exception:
            return False

    async def fetch_post_metrics(self, platform_post_id: str, **kwargs) -> MetricResult:
        """Fetch score and comment count from HN API.

        Endpoint: https://hacker-news.firebaseio.com/v0/item/{id}.json
        Returns: score (points) and descendants (total comment count)
        """
        async with httpx.AsyncClient() as client:
            resp = await client.get(
                f"{HN_API_BASE}/item/{platform_post_id}.json",
                timeout=10,
            )
            if resp.status_code != 200:
                raise ConnectionError(
                    f"HN API returned status {resp.status_code} for item {platform_post_id}"
                )

            data = resp.json()
            if data is None:
                raise ValueError(f"HN item {platform_post_id} not found")

            return MetricResult(
                likes=data.get("score", 0),
                comments=data.get("descendants", 0),
                fetched_via="api",
                snapshot_date=date.today(),
                custom_metrics={
                    "hn_type": data.get("type", ""),
                    "hn_by": data.get("by", ""),
                    "hn_time": data.get("time", 0),
                },
            )

    async def resolve_post_id(self, url: str) -> str | None:
        """Extract HN item ID from URL.

        Handles: news.ycombinator.com/item?id=12345
        """
        match = re.search(r"[?&]id=(\d+)", url)
        return match.group(1) if match else None

    def is_api_trackable(self) -> bool:
        return True
