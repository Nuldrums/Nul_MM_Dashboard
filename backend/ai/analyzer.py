"""Claude API analysis pipeline for campaign intelligence."""

import json
import logging
from datetime import datetime
from uuid import uuid4

import anthropic

from backend.ai.prompts import (
    CAMPAIGN_ANALYSIS_SYSTEM,
    CAMPAIGN_ANALYSIS_USER_TEMPLATE,
    CROSS_CAMPAIGN_SYSTEM,
    CROSS_CAMPAIGN_USER_TEMPLATE,
)

logger = logging.getLogger(__name__)


class CampaignAnalyzer:
    """Orchestrates AI analysis of campaign data via Claude API.

    1. Build context from campaign data + metric trajectories
    2. Query ChromaDB for historical context
    3. Send structured prompt to Claude API
    4. Parse and validate JSON response
    5. Return structured analysis result
    """

    def __init__(self, api_key: str = "", model: str = "claude-sonnet-4-20250514"):
        self.api_key = api_key
        self.model = model
        self.client: anthropic.Anthropic | None = None
        if api_key:
            self.client = anthropic.Anthropic(api_key=api_key)

    def _ensure_client(self):
        """Validate that the API client is configured."""
        if not self.api_key:
            raise ValueError(
                "No Anthropic API key configured. "
                "Set TRIKERI_ANTHROPIC_API_KEY in your .env file or environment."
            )
        if not self.client:
            self.client = anthropic.Anthropic(api_key=self.api_key)

    async def analyze_campaign(self, campaign_context: dict) -> dict:
        """Run AI analysis on a single campaign.

        Args:
            campaign_context: Dict with campaign info, posts, metrics, historical context.

        Returns:
            Structured analysis dict matching the AIAnalysis model schema.
        """
        try:
            self._ensure_client()
        except ValueError as e:
            return self._error_result(
                campaign_id=campaign_context.get("campaign_id"),
                analysis_type="campaign_daily",
                error=str(e),
            )

        # Build the user message from the template
        posts_data = self._format_posts_data(campaign_context.get("posts", []))
        historical_context = "\n".join(
            campaign_context.get("historical_context", [])
        ) or "No historical context available yet."

        duration_days = campaign_context.get("duration_days", "unknown")

        user_message = CAMPAIGN_ANALYSIS_USER_TEMPLATE.format(
            campaign_name=campaign_context.get("campaign_name", "Unknown"),
            product_name=campaign_context.get("product_name", "Unknown"),
            product_type=campaign_context.get("product_type", "Unknown"),
            goal=campaign_context.get("goal", "Not specified"),
            target_audience=campaign_context.get("target_audience", "Not specified"),
            duration_days=duration_days,
            posts_data=posts_data,
            historical_context=historical_context,
        )

        try:
            response = self.client.messages.create(
                model=self.model,
                max_tokens=4096,
                system=CAMPAIGN_ANALYSIS_SYSTEM,
                messages=[{"role": "user", "content": user_message}],
            )

            raw_text = response.content[0].text
            tokens_used = (response.usage.input_tokens or 0) + (response.usage.output_tokens or 0)

            # Parse the JSON response
            parsed = self._parse_json_response(raw_text)

            return {
                "id": str(uuid4()),
                "campaign_id": campaign_context.get("campaign_id"),
                "analysis_type": "campaign_daily",
                "summary": parsed.get("summary", "Analysis completed."),
                "effectiveness_score": parsed.get("effectiveness_score", 0),
                "top_performers": json.dumps(parsed.get("top_performers", [])),
                "underperformers": json.dumps(parsed.get("underperformers", [])),
                "patterns": json.dumps(parsed.get("patterns", [])),
                "recommendations": json.dumps(parsed.get("recommendations", [])),
                "meta_learning": parsed.get("meta_learning", {}),
                "raw_response": raw_text,
                "model_used": self.model,
                "tokens_used": tokens_used,
                "analyzed_at": datetime.utcnow(),
            }

        except anthropic.APIError as e:
            logger.error("Claude API error during campaign analysis: %s", e)
            return self._error_result(
                campaign_id=campaign_context.get("campaign_id"),
                analysis_type="campaign_daily",
                error=f"Claude API error: {e}",
            )
        except Exception as e:
            logger.error("Unexpected error during campaign analysis: %s", e)
            return self._error_result(
                campaign_id=campaign_context.get("campaign_id"),
                analysis_type="campaign_daily",
                error=f"Analysis error: {e}",
            )

    async def analyze_cross_campaign(self, campaigns_data: list[dict]) -> dict:
        """Run cross-campaign strategic analysis.

        Args:
            campaigns_data: List of campaign contexts with their metrics.

        Returns:
            Structured cross-campaign analysis dict.
        """
        try:
            self._ensure_client()
        except ValueError as e:
            return self._error_result(
                campaign_id=None,
                analysis_type="cross_campaign",
                error=str(e),
            )

        # Build combined campaigns summary
        campaigns_summary = self._format_campaigns_data(campaigns_data)
        historical_patterns = "No historical cross-campaign patterns available yet."

        user_message = CROSS_CAMPAIGN_USER_TEMPLATE.format(
            campaigns_data=campaigns_summary,
            historical_patterns=historical_patterns,
        )

        try:
            response = self.client.messages.create(
                model=self.model,
                max_tokens=4096,
                system=CROSS_CAMPAIGN_SYSTEM,
                messages=[{"role": "user", "content": user_message}],
            )

            raw_text = response.content[0].text
            tokens_used = (response.usage.input_tokens or 0) + (response.usage.output_tokens or 0)

            parsed = self._parse_json_response(raw_text)

            return {
                "id": str(uuid4()),
                "campaign_id": None,
                "analysis_type": "cross_campaign",
                "summary": parsed.get("summary", "Cross-campaign analysis completed."),
                "effectiveness_score": parsed.get("effectiveness_score", 0),
                "top_performers": json.dumps(parsed.get("top_performers", [])),
                "underperformers": json.dumps(parsed.get("underperformers", [])),
                "patterns": json.dumps(parsed.get("patterns", [])),
                "recommendations": json.dumps(parsed.get("recommendations", [])),
                "meta_learning": parsed.get("meta_learning", {}),
                "raw_response": raw_text,
                "model_used": self.model,
                "tokens_used": tokens_used,
                "analyzed_at": datetime.utcnow(),
            }

        except anthropic.APIError as e:
            logger.error("Claude API error during cross-campaign analysis: %s", e)
            return self._error_result(
                campaign_id=None,
                analysis_type="cross_campaign",
                error=f"Claude API error: {e}",
            )
        except Exception as e:
            logger.error("Unexpected error during cross-campaign analysis: %s", e)
            return self._error_result(
                campaign_id=None,
                analysis_type="cross_campaign",
                error=f"Analysis error: {e}",
            )

    async def build_campaign_context(
        self,
        campaign: dict,
        posts: list[dict],
        metrics: list[dict],
        historical_context: list[str] | None = None,
    ) -> dict:
        """Build the rich context document for a campaign analysis.

        Combines campaign metadata, post details with metric trajectories,
        and historical knowledge base context.
        """
        # Calculate duration
        duration_days = "unknown"
        start = campaign.get("start_date")
        end = campaign.get("end_date")
        if start and end:
            try:
                from datetime import date as date_type
                if isinstance(start, str):
                    start = date_type.fromisoformat(start)
                if isinstance(end, str):
                    end = date_type.fromisoformat(end)
                duration_days = (end - start).days
            except (ValueError, TypeError):
                pass
        elif start:
            from datetime import date as date_type
            try:
                if isinstance(start, str):
                    start = date_type.fromisoformat(start)
                duration_days = (date_type.today() - start).days
            except (ValueError, TypeError):
                pass

        # Enrich posts with their metric trajectories
        enriched_posts = []
        for post in posts:
            post_metrics = [m for m in metrics if m.get("post_id") == post.get("id")]
            # Sort by date for trajectory
            post_metrics.sort(key=lambda m: m.get("snapshot_date", ""))
            enriched_posts.append({
                "id": post.get("id"),
                "platform": post.get("platform"),
                "post_type": post.get("post_type"),
                "title": post.get("title", ""),
                "body_preview": post.get("body_preview", ""),
                "target_community": post.get("target_community", ""),
                "url": post.get("url", ""),
                "posted_at": str(post.get("posted_at", "")),
                "metric_trajectory": post_metrics,
            })

        return {
            "campaign_id": campaign.get("id"),
            "campaign_name": campaign.get("name"),
            "product_name": campaign.get("product_name", ""),
            "product_type": campaign.get("product_type", ""),
            "goal": campaign.get("goal", ""),
            "target_audience": campaign.get("target_audience", ""),
            "duration_days": duration_days,
            "posts": enriched_posts,
            "historical_context": historical_context or [],
        }

    def _format_posts_data(self, posts: list[dict]) -> str:
        """Format posts and their metrics into a readable string for the prompt."""
        if not posts:
            return "No posts tracked yet."

        lines = []
        for i, post in enumerate(posts, 1):
            lines.append(f"\n--- Post {i} ---")
            lines.append(f"Platform: {post.get('platform', 'unknown')}")
            lines.append(f"Type: {post.get('post_type', 'unknown')}")
            if post.get("title"):
                lines.append(f"Title: {post['title']}")
            if post.get("body_preview"):
                lines.append(f"Preview: {post['body_preview'][:200]}")
            if post.get("target_community"):
                lines.append(f"Community: {post['target_community']}")
            if post.get("url"):
                lines.append(f"URL: {post['url']}")
            if post.get("posted_at"):
                lines.append(f"Posted: {post['posted_at']}")

            trajectory = post.get("metric_trajectory", [])
            if trajectory:
                lines.append("Metric trajectory:")
                for snap in trajectory:
                    date_str = snap.get("snapshot_date", "?")
                    lines.append(
                        f"  {date_str}: views={snap.get('views', 0)} "
                        f"likes={snap.get('likes', 0)} "
                        f"comments={snap.get('comments', 0)} "
                        f"shares={snap.get('shares', 0)} "
                        f"saves={snap.get('saves', 0)} "
                        f"clicks={snap.get('clicks', 0)}"
                    )
            else:
                lines.append("No metrics recorded yet.")

        return "\n".join(lines)

    def _format_campaigns_data(self, campaigns_data: list[dict]) -> str:
        """Format multiple campaign contexts into a readable string."""
        if not campaigns_data:
            return "No campaign data available."

        sections = []
        for ctx in campaigns_data:
            section = [
                f"\n=== Campaign: {ctx.get('campaign_name', 'Unknown')} ===",
                f"Product: {ctx.get('product_name', '?')} ({ctx.get('product_type', '?')})",
                f"Goal: {ctx.get('goal', 'Not specified')}",
                f"Audience: {ctx.get('target_audience', 'Not specified')}",
                f"Duration: {ctx.get('duration_days', '?')} days",
                f"Posts: {len(ctx.get('posts', []))}",
            ]
            section.append(self._format_posts_data(ctx.get("posts", [])))
            sections.append("\n".join(section))

        return "\n\n".join(sections)

    def _parse_json_response(self, raw_text: str) -> dict:
        """Parse JSON from Claude's response, handling markdown code blocks."""
        text = raw_text.strip()

        # Strip markdown code fences if present
        if text.startswith("```"):
            # Remove first line (```json or ```)
            lines = text.split("\n")
            lines = lines[1:]  # drop opening fence
            # Remove closing fence
            if lines and lines[-1].strip() == "```":
                lines = lines[:-1]
            text = "\n".join(lines)

        try:
            return json.loads(text)
        except json.JSONDecodeError:
            logger.warning("Failed to parse Claude response as JSON, returning raw text as summary")
            return {
                "summary": text[:500],
                "top_performers": [],
                "underperformers": [],
                "patterns": [],
                "recommendations": [],
            }

    def _error_result(self, campaign_id: str | None, analysis_type: str, error: str) -> dict:
        """Build an error result dict."""
        return {
            "id": str(uuid4()),
            "campaign_id": campaign_id,
            "analysis_type": analysis_type,
            "summary": error,
            "effectiveness_score": 0,
            "top_performers": json.dumps([]),
            "underperformers": json.dumps([]),
            "patterns": json.dumps([]),
            "recommendations": json.dumps([]),
            "meta_learning": {},
            "raw_response": None,
            "model_used": self.model,
            "tokens_used": 0,
            "analyzed_at": datetime.utcnow(),
        }
