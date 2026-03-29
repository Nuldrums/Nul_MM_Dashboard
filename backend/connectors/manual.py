"""Manual entry fallback connector for platforms without API access."""

from backend.connectors.base import PlatformConnector, MetricResult


class ManualConnector(PlatformConnector):
    """Fallback for platforms that cannot be auto-tracked.

    Used for: Product Hunt, Instagram, LinkedIn, TikTok, and any
    platform where we don't have API access.
    """

    platform = "manual"

    async def validate_credentials(self) -> bool:
        """No credentials needed for manual entry."""
        return True

    async def fetch_post_metrics(self, platform_post_id: str, **kwargs) -> MetricResult:
        """Manual connector cannot fetch metrics automatically."""
        return MetricResult(fetched_via="manual")

    async def resolve_post_id(self, url: str) -> str | None:
        """For manual connectors, the URL itself serves as the identifier."""
        return url if url else None

    def is_api_trackable(self) -> bool:
        return False
