"""Application configuration using pydantic-settings."""

import os
import sys
from pathlib import Path
from pydantic_settings import BaseSettings


def _get_base_dir() -> str:
    """Get the base directory - handles both dev and PyInstaller bundled mode."""
    if getattr(sys, '_MEIPASS', None):
        # Running as PyInstaller bundle - _MEIPASS is the temp extraction dir
        return sys._MEIPASS
    # Dev mode - project root
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def _get_data_dir() -> str:
    """Get the data directory for persistent storage."""
    if getattr(sys, '_MEIPASS', None):
        # Packaged mode - store data in user's AppData
        appdata = os.environ.get('APPDATA', os.path.expanduser('~'))
        data_dir = os.path.join(appdata, 'TrikeriMarketingEngine', 'data')
    else:
        # Dev mode - store in backend/data
        data_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'data')
    os.makedirs(data_dir, exist_ok=True)
    return data_dir


def _find_env_file() -> str:
    """Find .env file in multiple locations."""
    candidates = [
        os.path.join(_get_base_dir(), '.env'),  # Bundled with exe or project root
        os.path.join(os.path.dirname(sys.executable), '.env'),  # Next to the exe
        os.path.join(os.getcwd(), '.env'),  # Current working directory
    ]
    if getattr(sys, '_MEIPASS', None):
        # Also check AppData for packaged mode
        appdata = os.environ.get('APPDATA', os.path.expanduser('~'))
        candidates.append(os.path.join(appdata, 'TrikeriMarketingEngine', '.env'))
    for path in candidates:
        if os.path.exists(path):
            return path
    return candidates[0]  # Default even if not found


class Settings(BaseSettings):
    """Global application settings, loaded from environment variables."""

    # Database
    data_dir: str = _get_data_dir()
    database_url: str = ""  # Computed in model_post_init

    # Server
    api_port: int = 31415
    cors_origins: list[str] = ["*"]

    # API Keys (loaded from env, empty by default)
    anthropic_api_key: str = ""
    reddit_client_id: str = ""
    reddit_client_secret: str = ""
    reddit_username: str = ""
    reddit_password: str = ""
    youtube_api_key: str = ""
    twitter_bearer_token: str = ""
    discord_bot_token: str = ""

    model_config = {
        "env_file": _find_env_file(),
        "extra": "ignore",
    }

    def model_post_init(self, __context) -> None:
        if not self.database_url:
            db_path = os.path.join(self.data_dir, "trikeri.db")
            self.database_url = f"sqlite+aiosqlite:///{db_path}"


settings = Settings()
