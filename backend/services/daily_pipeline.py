"""Full daily pipeline: fetch -> analyze -> embed -> update state."""

import json
import logging
from datetime import datetime
from dataclasses import dataclass, field

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from backend.config import settings
from backend.database.models import (
    Campaign, Post, MetricSnapshot, AIAnalysis, Product, SystemState,
)
from backend.services.metric_collector import MetricCollector
from backend.ai.analyzer import CampaignAnalyzer
from backend.ai.embedder import MarketingKnowledgeBase

logger = logging.getLogger(__name__)


@dataclass
class PipelineReport:
    """Summary of a full pipeline run."""
    started_at: datetime | None = None
    completed_at: datetime | None = None
    steps: list[dict] = field(default_factory=list)
    success: bool = True
    error: str | None = None


class DailyPipeline:
    """The full daily cycle:

    1. Fetch all metrics (via connectors)
    2. Run AI analysis on each active campaign
    3. Run cross-campaign analysis
    4. Embed new learnings into ChromaDB
    5. Update system state timestamps
    """

    def __init__(self):
        self.metric_collector = MetricCollector()
        self.analyzer = CampaignAnalyzer(api_key=settings.anthropic_api_key)
        self.knowledge_base = MarketingKnowledgeBase()

    async def run_full(self, session: AsyncSession) -> PipelineReport:
        """Execute the complete daily pipeline."""
        report = PipelineReport(started_at=datetime.utcnow())

        try:
            # Step 1: Fetch metrics
            logger.info("Pipeline step 1: Fetching metrics")
            fetch_result = await self.metric_collector.collect_all(session)
            report.steps.append({
                "step": "fetch_metrics",
                "fetched": fetch_result.fetched,
                "failed": fetch_result.failed,
                "skipped": fetch_result.skipped,
                "manual_needed": fetch_result.manual_needed,
                "errors": fetch_result.errors[:10],  # Cap error list
            })

            # Step 2: Per-campaign AI analysis
            logger.info("Pipeline step 2: Campaign analysis")
            campaigns_result = await session.execute(
                select(Campaign).where(Campaign.status == "active")
            )
            campaigns = campaigns_result.scalars().all()

            campaign_contexts = []
            for campaign in campaigns:
                try:
                    context = await self._build_full_context(session, campaign)
                    campaign_contexts.append(context)

                    analysis = await self.analyzer.analyze_campaign(context)

                    # Save to database
                    ai_analysis = AIAnalysis(
                        id=analysis["id"],
                        campaign_id=analysis["campaign_id"],
                        analysis_type=analysis["analysis_type"],
                        summary=analysis["summary"],
                        top_performers=analysis["top_performers"],
                        underperformers=analysis["underperformers"],
                        patterns=analysis["patterns"],
                        recommendations=analysis["recommendations"],
                        raw_response=analysis.get("raw_response"),
                        model_used=analysis["model_used"],
                        tokens_used=analysis["tokens_used"],
                        analyzed_at=analysis["analyzed_at"],
                    )
                    session.add(ai_analysis)

                    report.steps.append({
                        "step": "campaign_analysis",
                        "campaign_id": campaign.id,
                        "campaign_name": campaign.name,
                        "status": "completed",
                        "tokens_used": analysis["tokens_used"],
                    })

                    # Step 4a: Embed high-signal posts
                    await self._embed_high_signal_posts(analysis, context)

                    # Step 4b: Embed high-confidence patterns
                    await self._embed_patterns(analysis)

                except Exception as e:
                    logger.error("Analysis failed for campaign %s: %s", campaign.id, e)
                    report.steps.append({
                        "step": "campaign_analysis",
                        "campaign_id": campaign.id,
                        "campaign_name": campaign.name,
                        "status": "failed",
                        "error": str(e),
                    })

            # Step 3: Cross-campaign analysis if >1 active campaign
            if len(campaign_contexts) > 1:
                logger.info("Pipeline step 3: Cross-campaign analysis")
                try:
                    cross_analysis = await self.analyzer.analyze_cross_campaign(
                        campaign_contexts
                    )

                    cross_record = AIAnalysis(
                        id=cross_analysis["id"],
                        campaign_id=None,
                        analysis_type=cross_analysis["analysis_type"],
                        summary=cross_analysis["summary"],
                        top_performers=cross_analysis["top_performers"],
                        underperformers=cross_analysis["underperformers"],
                        patterns=cross_analysis["patterns"],
                        recommendations=cross_analysis["recommendations"],
                        raw_response=cross_analysis.get("raw_response"),
                        model_used=cross_analysis["model_used"],
                        tokens_used=cross_analysis["tokens_used"],
                        analyzed_at=cross_analysis["analyzed_at"],
                    )
                    session.add(cross_record)

                    report.steps.append({
                        "step": "cross_campaign_analysis",
                        "status": "completed",
                        "tokens_used": cross_analysis["tokens_used"],
                    })

                    # Embed cross-campaign patterns
                    await self._embed_patterns(cross_analysis)

                except Exception as e:
                    logger.error("Cross-campaign analysis failed: %s", e)
                    report.steps.append({
                        "step": "cross_campaign_analysis",
                        "status": "failed",
                        "error": str(e),
                    })
            else:
                report.steps.append({
                    "step": "cross_campaign_analysis",
                    "status": "skipped",
                    "reason": f"Only {len(campaign_contexts)} active campaign(s)",
                })

            # Step 5: Update system state
            now = datetime.utcnow()
            for key in ["last_metric_fetch", "last_ai_analysis"]:
                state_result = await session.execute(
                    select(SystemState).where(SystemState.key == key)
                )
                state = state_result.scalar_one_or_none()
                if state:
                    state.value = now.isoformat()
                    state.updated_at = now
                else:
                    session.add(SystemState(
                        key=key, value=now.isoformat(), updated_at=now,
                    ))

            await session.commit()
            report.success = True
            logger.info("Pipeline completed successfully with %d steps", len(report.steps))

        except Exception as e:
            report.success = False
            report.error = str(e)
            logger.error("Pipeline failed: %s", e)

        report.completed_at = datetime.utcnow()
        return report

    async def run_analysis_only(self, session: AsyncSession) -> PipelineReport:
        """Run only the analysis portion (assumes metrics are fresh)."""
        report = PipelineReport(started_at=datetime.utcnow())

        try:
            campaigns_result = await session.execute(
                select(Campaign).where(Campaign.status == "active")
            )
            campaigns = campaigns_result.scalars().all()

            campaign_contexts = []
            for campaign in campaigns:
                try:
                    context = await self._build_full_context(session, campaign)
                    campaign_contexts.append(context)

                    analysis = await self.analyzer.analyze_campaign(context)

                    ai_analysis = AIAnalysis(
                        id=analysis["id"],
                        campaign_id=analysis["campaign_id"],
                        analysis_type=analysis["analysis_type"],
                        summary=analysis["summary"],
                        top_performers=analysis["top_performers"],
                        underperformers=analysis["underperformers"],
                        patterns=analysis["patterns"],
                        recommendations=analysis["recommendations"],
                        raw_response=analysis.get("raw_response"),
                        model_used=analysis["model_used"],
                        tokens_used=analysis["tokens_used"],
                        analyzed_at=analysis["analyzed_at"],
                    )
                    session.add(ai_analysis)

                    report.steps.append({
                        "step": "campaign_analysis",
                        "campaign_id": campaign.id,
                        "status": "completed",
                    })
                except Exception as e:
                    logger.error("Analysis failed for campaign %s: %s", campaign.id, e)
                    report.steps.append({
                        "step": "campaign_analysis",
                        "campaign_id": campaign.id,
                        "status": "failed",
                        "error": str(e),
                    })

            # Cross-campaign if >1
            if len(campaign_contexts) > 1:
                try:
                    cross = await self.analyzer.analyze_cross_campaign(campaign_contexts)
                    session.add(AIAnalysis(
                        id=cross["id"],
                        campaign_id=None,
                        analysis_type=cross["analysis_type"],
                        summary=cross["summary"],
                        top_performers=cross["top_performers"],
                        underperformers=cross["underperformers"],
                        patterns=cross["patterns"],
                        recommendations=cross["recommendations"],
                        raw_response=cross.get("raw_response"),
                        model_used=cross["model_used"],
                        tokens_used=cross["tokens_used"],
                        analyzed_at=cross["analyzed_at"],
                    ))
                    report.steps.append({
                        "step": "cross_campaign_analysis",
                        "status": "completed",
                    })
                except Exception as e:
                    report.steps.append({
                        "step": "cross_campaign_analysis",
                        "status": "failed",
                        "error": str(e),
                    })

            # Update state
            now = datetime.utcnow()
            state_result = await session.execute(
                select(SystemState).where(SystemState.key == "last_ai_analysis")
            )
            state = state_result.scalar_one_or_none()
            if state:
                state.value = now.isoformat()
                state.updated_at = now
            else:
                session.add(SystemState(
                    key="last_ai_analysis", value=now.isoformat(), updated_at=now,
                ))

            await session.commit()
            report.success = True

        except Exception as e:
            report.success = False
            report.error = str(e)

        report.completed_at = datetime.utcnow()
        return report

    async def _build_full_context(self, session: AsyncSession, campaign: Campaign) -> dict:
        """Build the full context for a campaign including product info, posts, and metrics."""
        # Get product info
        product_result = await session.execute(
            select(Product).where(Product.id == campaign.product_id)
        )
        product = product_result.scalar_one_or_none()

        # Get all posts for this campaign
        posts_result = await session.execute(
            select(Post).where(Post.campaign_id == campaign.id)
        )
        posts = posts_result.scalars().all()

        # Get all metric snapshots for these posts
        post_ids = [p.id for p in posts]
        metrics = []
        if post_ids:
            metrics_result = await session.execute(
                select(MetricSnapshot).where(
                    MetricSnapshot.post_id.in_(post_ids)
                ).order_by(MetricSnapshot.snapshot_date.asc())
            )
            metrics = metrics_result.scalars().all()

        # Convert to dicts for the analyzer
        posts_data = [
            {
                "id": p.id,
                "platform": p.platform,
                "post_type": p.post_type,
                "title": p.title,
                "body_preview": p.body_preview,
                "target_community": p.target_community,
                "url": p.url,
                "posted_at": p.posted_at.isoformat() if p.posted_at else None,
            }
            for p in posts
        ]

        metrics_data = [
            {
                "post_id": m.post_id,
                "snapshot_date": m.snapshot_date.isoformat() if m.snapshot_date else None,
                "views": m.views,
                "impressions": m.impressions,
                "likes": m.likes,
                "dislikes": m.dislikes,
                "comments": m.comments,
                "shares": m.shares,
                "saves": m.saves,
                "clicks": m.clicks,
            }
            for m in metrics
        ]

        # Query ChromaDB for historical context
        historical_context = []
        try:
            await self.knowledge_base.initialize()
            platforms = list({p.platform for p in posts})
            similar = await self.knowledge_base.query_similar_campaigns(
                product_type=product.type if product else "unknown",
                platforms=platforms,
                goal=campaign.goal or "",
                n=3,
            )
            historical_context = [doc["document"] for doc in similar if doc.get("document")]
        except Exception as e:
            logger.debug("Could not query knowledge base: %s", e)

        campaign_dict = {
            "id": campaign.id,
            "name": campaign.name,
            "goal": campaign.goal,
            "target_audience": campaign.target_audience,
            "start_date": campaign.start_date,
            "end_date": campaign.end_date,
            "product_name": product.name if product else "Unknown",
            "product_type": product.type if product else "unknown",
        }

        return await self.analyzer.build_campaign_context(
            campaign=campaign_dict,
            posts=posts_data,
            metrics=metrics_data,
            historical_context=historical_context,
        )

    async def _embed_high_signal_posts(self, analysis: dict, context: dict):
        """Embed posts with scores >75 or <25 into ChromaDB."""
        try:
            await self.knowledge_base.initialize()
        except Exception:
            return

        # Check top performers (high signal positive)
        top_performers = analysis.get("top_performers", "[]")
        if isinstance(top_performers, str):
            try:
                top_performers = json.loads(top_performers)
            except (json.JSONDecodeError, TypeError):
                top_performers = []

        for performer in top_performers:
            if not isinstance(performer, dict):
                continue
            score = performer.get("score", 0)
            if score > 75:
                post_id = performer.get("post_id", "")
                # Find matching post in context for extra info
                post_info = next(
                    (p for p in context.get("posts", []) if p.get("id") == post_id),
                    {},
                )
                await self.knowledge_base.embed_post_insight(post_id, {
                    **performer,
                    "platform": post_info.get("platform", "unknown"),
                    "post_type": post_info.get("post_type", "unknown"),
                    "target_community": post_info.get("target_community", ""),
                    "title": post_info.get("title", ""),
                })

        # Check underperformers (high signal negative)
        underperformers = analysis.get("underperformers", "[]")
        if isinstance(underperformers, str):
            try:
                underperformers = json.loads(underperformers)
            except (json.JSONDecodeError, TypeError):
                underperformers = []

        for performer in underperformers:
            if not isinstance(performer, dict):
                continue
            score = performer.get("score", 50)
            if score < 25:
                post_id = performer.get("post_id", "")
                post_info = next(
                    (p for p in context.get("posts", []) if p.get("id") == post_id),
                    {},
                )
                await self.knowledge_base.embed_post_insight(post_id, {
                    **performer,
                    "platform": post_info.get("platform", "unknown"),
                    "post_type": post_info.get("post_type", "unknown"),
                    "target_community": post_info.get("target_community", ""),
                    "title": post_info.get("title", ""),
                })

    async def _embed_patterns(self, analysis: dict):
        """Embed high-confidence patterns into ChromaDB."""
        try:
            await self.knowledge_base.initialize()
        except Exception:
            return

        patterns = analysis.get("patterns", "[]")
        if isinstance(patterns, str):
            try:
                patterns = json.loads(patterns)
            except (json.JSONDecodeError, TypeError):
                patterns = []

        analysis_id = analysis.get("id", "unknown")
        for pattern in patterns:
            if not isinstance(pattern, dict):
                continue
            if pattern.get("confidence") == "high":
                await self.knowledge_base.embed_pattern(pattern, analysis_id)
