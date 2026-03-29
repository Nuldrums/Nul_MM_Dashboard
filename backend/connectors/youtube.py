"""YouTube Data API v3 connector."""

import logging
import re
from datetime import date

from googleapiclient.discovery import build
from googleapiclient.errors import HttpError

from backend.connectors.base import PlatformConnector, MetricResult

logger = logging.getLogger(__name__)


class YouTubeConnector(PlatformConnector):
    platform = "youtube"

    def __init__(self, api_key: str = ""):
        self.api_key = api_key
        self._service = None

    def _get_service(self):
        if self._service is None:
            if not self.api_key:
                raise ValueError(
                    "YouTube API key not configured. "
                    "Set TRIKERI_YOUTUBE_API_KEY in your .env file."
                )
            self._service = build("youtube", "v3", developerKey=self.api_key)
        return self._service

    async def validate_credentials(self) -> bool:
        """Test YouTube API key validity."""
        if not self.api_key:
            return False
        try:
            service = self._get_service()
            # Lightweight call to check the key
            request = service.videos().list(part="id", id="dQw4w9WgXcQ")
            request.execute()
            return True
        except Exception as e:
            logger.warning("YouTube credential validation failed: %s", e)
            return False

    async def fetch_post_metrics(self, platform_post_id: str, **kwargs) -> MetricResult:
        """Fetch metrics for a YouTube video.

        Mapping:
          views = viewCount
          likes = likeCount
          comments = commentCount
        """
        service = self._get_service()

        try:
            request = service.videos().list(
                part="statistics",
                id=platform_post_id,
            )
            response = request.execute()

            items = response.get("items", [])
            if not items:
                raise ValueError(f"YouTube video {platform_post_id} not found")

            stats = items[0].get("statistics", {})

            return MetricResult(
                views=int(stats.get("viewCount", 0)),
                likes=int(stats.get("likeCount", 0)),
                dislikes=int(stats.get("dislikeCount", 0)),
                comments=int(stats.get("commentCount", 0)),
                saves=int(stats.get("favoriteCount", 0)),
                fetched_via="api",
                snapshot_date=date.today(),
                custom_metrics={
                    "view_count_raw": stats.get("viewCount"),
                    "like_count_raw": stats.get("likeCount"),
                },
            )

        except HttpError as e:
            logger.error("YouTube API error: %s", e)
            raise ConnectionError(f"YouTube API error: {e}") from e

    async def resolve_post_id(self, url: str) -> str | None:
        """Extract YouTube video ID from URL.

        Handles:
          youtube.com/watch?v=ID
          youtube.com/shorts/ID
          youtu.be/ID
        """
        patterns = [
            r"(?:v=|/shorts/)([a-zA-Z0-9_-]{11})",
            r"youtu\.be/([a-zA-Z0-9_-]{11})",
        ]
        for pattern in patterns:
            match = re.search(pattern, url)
            if match:
                return match.group(1)
        return None

    def is_api_trackable(self) -> bool:
        return True
