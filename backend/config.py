"""Application configuration using pydantic-settings."""

import os
from pathlib import Path
from pydantic_settings import BaseSettings


class Settings(BaseSettings):
    """Global application settings, loaded from environment variables."""

    # Database
    data_dir: str = os.path.join(os.path.dirname(os.path.abspath(__file__)), "data")
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

    model_config = {"env_prefix": "TRIKERI_", "env_file": ".env", "extra": "ignore"}

    def model_post_init(self, __context) -> None:
        if not self.database_url:
            db_path = os.path.join(self.data_dir, "trikeri.db")
            self.database_url = f"sqlite+aiosqlite:///{db_path}"


settings = Settings()
