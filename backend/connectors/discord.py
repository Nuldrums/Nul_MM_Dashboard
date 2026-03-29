"""Discord connector - manual entry only for MVP."""

import logging

from backend.connectors.base import PlatformConnector, MetricResult

logger = logging.getLogger(__name__)


class DiscordConnector(PlatformConnector):
    """Discord has no public analytics API for servers you don't own.

    Tier 1 (MVP): Manual entry only.
    Tier 2 (future): Personal bot in your own server.
    Tier 3 (future): Webhook listener.
    """

    platform = "discord"

    def __init__(self, bot_token: str = ""):
        self.bot_token = bot_token

    async def validate_credentials(self) -> bool:
        """Discord bot token validation - not used in MVP."""
        # MVP is manual-only, no bot token needed
        return True

    async def fetch_post_metrics(self, platform_post_id: str, **kwargs) -> MetricResult:
        """Discord metrics require manual entry for MVP.

        Returns a manual MetricResult so the collector doesn't error out.
        """
        return MetricResult(fetched_via="manual")

    async def resolve_post_id(self, url: str) -> str | None:
        """Extract Discord message ID from URL.

        Handles: discord.com/channels/server_id/channel_id/message_id
        """
        parts = url.rstrip("/").split("/")
        if len(parts) >= 3 and "channels" in url:
            return parts[-1]  # message_id
        return None

    def is_api_trackable(self) -> bool:
        """Discord is manual-only for MVP."""
        return False
