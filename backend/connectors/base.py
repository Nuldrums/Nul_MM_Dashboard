"""Abstract base class for all platform connectors."""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import date


@dataclass
class MetricResult:
    """Normalized metric data returned by connectors."""
    views: int = 0
    impressions: int = 0
    likes: int = 0
    dislikes: int = 0
    comments: int = 0
    shares: int = 0
    saves: int = 0
    clicks: int = 0
    watch_time_seconds: int | None = None
    followers_gained: int = 0
    custom_metrics: dict = field(default_factory=dict)
    fetched_via: str = "api"
    snapshot_date: date | None = None


class PlatformConnector(ABC):
    """Interface that all platform connectors must implement."""

    platform: str

    @abstractmethod
    async def validate_credentials(self) -> bool:
        """Test if stored API credentials are valid."""
        ...

    @abstractmethod
    async def fetch_post_metrics(self, platform_post_id: str, **kwargs) -> MetricResult:
        """Fetch current metrics for a single post by its platform-native ID."""
        ...

    @abstractmethod
    async def resolve_post_id(self, url: str) -> str | None:
        """Extract platform-native post ID from a URL."""
        ...

    @abstractmethod
    def is_api_trackable(self) -> bool:
        """Whether this platform can be auto-tracked via API."""
        ...
