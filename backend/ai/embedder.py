"""ChromaDB embedding and retrieval for the marketing knowledge base."""

import json
import logging
from datetime import datetime

import chromadb

logger = logging.getLogger(__name__)

COLLECTION_NAME = "trikeri_marketing_knowledge"


class MarketingKnowledgeBase:
    """Vector knowledge base using ChromaDB for semantic search.

    Stores and retrieves:
    1. Campaign completion summaries
    2. High-signal post analyses (score >75 or <25)
    3. High-confidence pattern observations
    4. Weekly cross-campaign digests
    """

    def __init__(self, persist_dir: str = "data/chromadb"):
        self.persist_dir = persist_dir
        self._client: chromadb.PersistentClient | None = None
        self._collection: chromadb.Collection | None = None

    async def initialize(self):
        """Initialize ChromaDB client and collection."""
        if self._client is not None:
            return
        try:
            self._client = chromadb.PersistentClient(path=self.persist_dir)
            self._collection = self._client.get_or_create_collection(
                name=COLLECTION_NAME,
                metadata={"hnsw:space": "cosine"},
            )
            logger.info(
                "ChromaDB initialized at %s with %d documents",
                self.persist_dir,
                self._collection.count(),
            )
        except Exception as e:
            logger.error("Failed to initialize ChromaDB: %s", e)
            self._client = None
            self._collection = None
            raise

    def _ensure_initialized(self):
        """Raise if the collection is not ready."""
        if self._collection is None:
            raise RuntimeError(
                "MarketingKnowledgeBase not initialized. Call initialize() first."
            )

    async def query_similar(
        self, query_text: str, n: int = 5, filters: dict | None = None
    ) -> list[dict]:
        """Semantic search the knowledge base.

        Args:
            query_text: Natural language query.
            n: Number of results to return.
            filters: Optional ChromaDB where-filter dict.

        Returns:
            List of dicts with id, document, metadata, and distance.
        """
        self._ensure_initialized()
        if self._collection.count() == 0:
            return []

        kwargs: dict = {
            "query_texts": [query_text],
            "n_results": min(n, self._collection.count()),
        }
        if filters:
            kwargs["where"] = filters

        try:
            results = self._collection.query(**kwargs)
        except Exception as e:
            logger.error("ChromaDB query failed: %s", e)
            return []

        output = []
        ids = results.get("ids", [[]])[0]
        documents = results.get("documents", [[]])[0]
        metadatas = results.get("metadatas", [[]])[0]
        distances = results.get("distances", [[]])[0]

        for i, doc_id in enumerate(ids):
            output.append({
                "id": doc_id,
                "document": documents[i] if i < len(documents) else "",
                "metadata": metadatas[i] if i < len(metadatas) else {},
                "distance": distances[i] if i < len(distances) else 1.0,
            })

        return output

    async def query_similar_campaigns(
        self,
        product_type: str,
        platforms: list[str],
        goal: str,
        n: int = 5,
    ) -> list[dict]:
        """Query for historically similar campaigns.

        Returns the most relevant past learnings for a given product type,
        platform set, and campaign goal.
        """
        self._ensure_initialized()
        query_text = (
            f"Campaign for {product_type} product on {', '.join(platforms)} "
            f"with goal: {goal}"
        )
        return await self.query_similar(
            query_text=query_text,
            n=n,
            filters={"doc_type": "campaign_completion"},
        )

    async def embed_campaign_completion(
        self, campaign_id: str, campaign_data: dict
    ):
        """Build and store a rich completion document for a finished campaign.

        Args:
            campaign_id: The campaign's unique ID.
            campaign_data: Dict containing campaign info, posts, metrics, analysis.
        """
        self._ensure_initialized()

        # Build a narrative document from campaign data
        doc_parts = [
            f"Campaign: {campaign_data.get('name', 'Unknown')}",
            f"Product: {campaign_data.get('product_name', '?')} ({campaign_data.get('product_type', '?')})",
            f"Goal: {campaign_data.get('goal', 'Not specified')}",
            f"Target Audience: {campaign_data.get('target_audience', 'Not specified')}",
            f"Status: {campaign_data.get('status', 'unknown')}",
        ]

        # Summarize post performance
        posts = campaign_data.get("posts", [])
        if posts:
            platforms = list({p.get("platform", "unknown") for p in posts})
            doc_parts.append(f"Platforms used: {', '.join(platforms)}")
            doc_parts.append(f"Total posts: {len(posts)}")

        # Include analysis summary if available
        analysis = campaign_data.get("analysis", {})
        if analysis.get("summary"):
            doc_parts.append(f"Analysis: {analysis['summary']}")

        # Include recommendations
        recs = analysis.get("recommendations", [])
        if recs:
            rec_texts = []
            for r in recs[:5]:
                if isinstance(r, dict):
                    rec_texts.append(r.get("action", str(r)))
                else:
                    rec_texts.append(str(r))
            doc_parts.append(f"Key recommendations: {'; '.join(rec_texts)}")

        document = "\n".join(doc_parts)
        doc_id = f"campaign_{campaign_id}"

        metadata = {
            "doc_type": "campaign_completion",
            "campaign_id": campaign_id,
            "product_type": campaign_data.get("product_type", "unknown"),
            "embedded_at": datetime.utcnow().isoformat(),
        }

        self._collection.upsert(
            ids=[doc_id],
            documents=[document],
            metadatas=[metadata],
        )
        logger.info("Embedded campaign completion for %s", campaign_id)

    async def embed_post_insight(self, post_id: str, analysis: dict):
        """Store a strong-signal post learning (score >75 or <25).

        Args:
            post_id: The post's unique ID.
            analysis: Dict with score, reasoning, platform, post_type, etc.
        """
        self._ensure_initialized()

        doc_parts = [
            f"Post insight for {analysis.get('platform', 'unknown')} {analysis.get('post_type', 'post')}",
            f"Score: {analysis.get('score', 0)}/100",
            f"Reasoning: {analysis.get('reasoning', 'No reasoning provided')}",
        ]

        if analysis.get("target_community"):
            doc_parts.append(f"Community: {analysis['target_community']}")
        if analysis.get("title"):
            doc_parts.append(f"Title: {analysis['title']}")

        document = "\n".join(doc_parts)
        doc_id = f"post_insight_{post_id}"

        metadata = {
            "doc_type": "post_insight",
            "post_id": post_id,
            "platform": analysis.get("platform", "unknown"),
            "score": analysis.get("score", 0),
            "signal": "strong_positive" if analysis.get("score", 0) > 75 else "strong_negative",
            "embedded_at": datetime.utcnow().isoformat(),
        }

        self._collection.upsert(
            ids=[doc_id],
            documents=[document],
            metadatas=[metadata],
        )
        logger.info("Embedded post insight for %s (score=%s)", post_id, analysis.get("score"))

    async def embed_pattern(self, pattern: dict, analysis_id: str):
        """Store a high-confidence pattern observation.

        Args:
            pattern: Dict with pattern, confidence, evidence, actionable_insight.
            analysis_id: The AI analysis ID that produced this pattern.
        """
        self._ensure_initialized()

        document = (
            f"Pattern: {pattern.get('pattern', '')}\n"
            f"Confidence: {pattern.get('confidence', 'unknown')}\n"
            f"Evidence: {pattern.get('evidence', '')}\n"
            f"Insight: {pattern.get('actionable_insight', '')}"
        )
        doc_id = f"pattern_{analysis_id}_{hash(pattern.get('pattern', '')) % 10000}"

        metadata = {
            "doc_type": "pattern",
            "confidence": pattern.get("confidence", "unknown"),
            "analysis_id": analysis_id,
            "embedded_at": datetime.utcnow().isoformat(),
        }

        self._collection.upsert(
            ids=[doc_id],
            documents=[document],
            metadatas=[metadata],
        )
        logger.info("Embedded pattern from analysis %s", analysis_id)

    async def get_stats(self) -> dict:
        """Return collection stats."""
        if self._collection is None:
            return {
                "initialized": False,
                "total_documents": 0,
                "coverage": {
                    "campaigns_embedded": 0,
                    "posts_embedded": 0,
                    "patterns_stored": 0,
                },
            }

        total = self._collection.count()

        # Count by doc_type
        campaigns_count = 0
        posts_count = 0
        patterns_count = 0

        if total > 0:
            try:
                # Query all metadatas to count types
                all_data = self._collection.get(include=["metadatas"])
                for meta in all_data.get("metadatas", []):
                    doc_type = meta.get("doc_type", "")
                    if doc_type == "campaign_completion":
                        campaigns_count += 1
                    elif doc_type == "post_insight":
                        posts_count += 1
                    elif doc_type == "pattern":
                        patterns_count += 1
            except Exception as e:
                logger.warning("Could not count doc types: %s", e)

        return {
            "initialized": True,
            "total_documents": total,
            "coverage": {
                "campaigns_embedded": campaigns_count,
                "posts_embedded": posts_count,
                "patterns_stored": patterns_count,
            },
        }
