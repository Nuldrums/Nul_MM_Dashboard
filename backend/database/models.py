"""SQLAlchemy 2.0 models matching the Trikeri schema."""

from datetime import datetime, date
from sqlalchemy import (
    Column, Text, Integer, Float, Date, DateTime, ForeignKey, UniqueConstraint,
    func
)
from sqlalchemy.orm import DeclarativeBase, relationship


class Base(DeclarativeBase):
    pass


class Product(Base):
    __tablename__ = "products"

    id = Column(Text, primary_key=True)
    name = Column(Text, nullable=False)
    type = Column(Text, nullable=False)
    description = Column(Text)
    url = Column(Text)
    price = Column(Float)
    tags = Column(Text)  # JSON array
    created_at = Column(DateTime, default=func.now())

    campaigns = relationship("Campaign", back_populates="product")


class Campaign(Base):
    __tablename__ = "campaigns"

    id = Column(Text, primary_key=True)
    product_id = Column(Text, ForeignKey("products.id"), nullable=False)
    name = Column(Text, nullable=False)
    status = Column(Text, default="active")
    goal = Column(Text)
    target_audience = Column(Text)
    start_date = Column(Date)
    end_date = Column(Date)
    notes = Column(Text)
    created_at = Column(DateTime, default=func.now())
    updated_at = Column(DateTime, default=func.now(), onupdate=func.now())

    product = relationship("Product", back_populates="campaigns")
    posts = relationship("Post", back_populates="campaign")
    ai_analyses = relationship("AIAnalysis", back_populates="campaign")


class Post(Base):
    __tablename__ = "posts"

    id = Column(Text, primary_key=True)
    campaign_id = Column(Text, ForeignKey("campaigns.id"), nullable=False)
    platform = Column(Text, nullable=False)
    post_type = Column(Text, nullable=False)
    platform_post_id = Column(Text)
    url = Column(Text)
    title = Column(Text)
    body_preview = Column(Text)
    target_community = Column(Text)
    posted_at = Column(DateTime)
    tags = Column(Text)  # JSON array
    is_api_tracked = Column(Integer, default=0)
    created_at = Column(DateTime, default=func.now())

    campaign = relationship("Campaign", back_populates="posts")
    metric_snapshots = relationship("MetricSnapshot", back_populates="post")


class MetricSnapshot(Base):
    __tablename__ = "metric_snapshots"

    id = Column(Integer, primary_key=True, autoincrement=True)
    post_id = Column(Text, ForeignKey("posts.id"), nullable=False)
    snapshot_date = Column(Date, nullable=False)
    views = Column(Integer, default=0)
    impressions = Column(Integer, default=0)
    likes = Column(Integer, default=0)
    dislikes = Column(Integer, default=0)
    comments = Column(Integer, default=0)
    shares = Column(Integer, default=0)
    saves = Column(Integer, default=0)
    clicks = Column(Integer, default=0)
    watch_time_seconds = Column(Integer)
    followers_gained = Column(Integer, default=0)
    custom_metrics = Column(Text)  # JSON
    fetched_via = Column(Text, default="manual")
    created_at = Column(DateTime, default=func.now())

    __table_args__ = (
        UniqueConstraint("post_id", "snapshot_date", name="uq_post_snapshot_date"),
    )

    post = relationship("Post", back_populates="metric_snapshots")


class AIAnalysis(Base):
    __tablename__ = "ai_analyses"

    id = Column(Text, primary_key=True)
    campaign_id = Column(Text, ForeignKey("campaigns.id"))
    analysis_type = Column(Text, nullable=False)
    summary = Column(Text, nullable=False)
    top_performers = Column(Text)  # JSON
    underperformers = Column(Text)  # JSON
    patterns = Column(Text)  # JSON
    recommendations = Column(Text)  # JSON
    raw_response = Column(Text)
    model_used = Column(Text)
    tokens_used = Column(Integer)
    analyzed_at = Column(DateTime, default=func.now())

    campaign = relationship("Campaign", back_populates="ai_analyses")


class SystemState(Base):
    __tablename__ = "system_state"

    key = Column(Text, primary_key=True)
    value = Column(Text)
    updated_at = Column(DateTime, default=func.now())


class PlatformConfig(Base):
    __tablename__ = "platform_configs"

    platform = Column(Text, primary_key=True)
    credentials = Column(Text)  # JSON, encrypted
    is_enabled = Column(Integer, default=0)
    rate_limit_remaining = Column(Integer)
    last_fetched_at = Column(DateTime)
    config = Column(Text)  # JSON
