# Trikeri Marketing Engine — Architecture & Agent Build Plan

## Overview

A local-first Tauri desktop application for managing multi-platform marketing campaigns. The system tracks posts across Reddit, X/Twitter, YouTube, Discord, and other platforms — auto-fetching engagement metrics via APIs, running daily AI analysis through Claude, and building an internal vector knowledge base that learns what promotion strategies work for which product types.

**Stack:** Tauri 2.x (Rust shell) → React + TypeScript (frontend) → Python FastAPI (sidecar backend) → SQLite (structured data) → ChromaDB (vector knowledge base)

---

## Agent Team Structure

Six specialized agents, each responsible for a vertical slice of the system. They share a common data contract (defined by Agent 1) and can be developed in parallel after the foundation is laid.

```
Agent 1: Architect        → Data models, API contracts, project scaffold
Agent 2: Platform Connectors  → Reddit, X, YouTube, Discord API integrations
Agent 3: Backend Core     → FastAPI server, SQLite, scheduling, sidecar config
Agent 4: AI Brain         → Claude analysis pipeline, ChromaDB, embeddings
Agent 5: Frontend Dashboard    → React UI, charts, campaign management
Agent 6: Packaging & Config    → Tauri shell, settings, color themes, builds
```

### Dependency Graph

```
Phase 1 (Foundation):    Agent 1 → shared by all
Phase 2 (Parallel):      Agent 2 + Agent 3 + Agent 5 (all build on Agent 1's contracts)
Phase 3 (Integration):   Agent 4 (needs Agent 2 + 3 data flowing)
Phase 4 (Ship):          Agent 6 (wraps everything into distributable app)
```

---

## Agent 1: Architect

**Responsibility:** Define every data model, API endpoint contract, file structure, and interface boundary before anyone else writes code.

### Project Structure

```
trikeri-marketing-engine/
├── src-tauri/                    # Tauri Rust shell
│   ├── src/
│   │   └── main.rs              # Sidecar launcher, window config
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                          # React frontend
│   ├── App.tsx
│   ├── main.tsx
│   ├── theme/
│   │   ├── themes.json          # All color schemes
│   │   └── ThemeProvider.tsx
│   ├── pages/
│   │   ├── Dashboard.tsx        # Overview: all campaigns
│   │   ├── CampaignDetail.tsx   # Single campaign deep-dive
│   │   ├── PostComposer.tsx     # Log a new post
│   │   ├── Analytics.tsx        # Cross-campaign analytics
│   │   ├── AIInsights.tsx       # AI summaries & recommendations
│   │   └── Settings.tsx         # API keys, theme switcher, config
│   ├── components/
│   │   ├── CampaignCard.tsx
│   │   ├── PostRow.tsx
│   │   ├── MetricSparkline.tsx
│   │   ├── PlatformBadge.tsx
│   │   ├── EngagementChart.tsx
│   │   ├── AIRecommendation.tsx
│   │   └── ThemeSwitcher.tsx
│   ├── hooks/
│   │   ├── useApi.ts            # Fetch wrapper for Python backend
│   │   ├── useCampaigns.ts
│   │   ├── useMetrics.ts
│   │   └── useTheme.ts
│   └── lib/
│       ├── types.ts             # Shared TypeScript types
│       └── constants.ts
├── backend/                      # Python FastAPI sidecar
│   ├── main.py                  # FastAPI app + startup hooks
│   ├── config.py                # Settings, API keys, paths
│   ├── database/
│   │   ├── models.py            # SQLAlchemy models
│   │   ├── connection.py        # SQLite connection manager
│   │   └── migrations.py        # Schema versioning
│   ├── routers/
│   │   ├── campaigns.py         # CRUD for campaigns
│   │   ├── posts.py             # CRUD for posts
│   │   ├── metrics.py           # Metric snapshots + fetch triggers
│   │   ├── analytics.py         # Aggregated analytics endpoints
│   │   ├── ai_analysis.py       # AI insight endpoints
│   │   └── settings.py          # Config management
│   ├── connectors/
│   │   ├── base.py              # Abstract connector interface
│   │   ├── reddit.py            # Reddit API (PRAW)
│   │   ├── youtube.py           # YouTube Data API v3
│   │   ├── twitter.py           # X API v2
│   │   ├── discord.py           # Discord bot / manual fallback
│   │   └── manual.py            # Manual entry fallback for any platform
│   ├── ai/
│   │   ├── analyzer.py          # Claude API analysis pipeline
│   │   ├── embedder.py          # ChromaDB embedding + retrieval
│   │   ├── prompts.py           # Structured prompt templates
│   │   └── scheduler.py         # 24-hour analysis trigger logic
│   └── services/
│       ├── metric_collector.py  # Orchestrates all connectors
│       └── daily_pipeline.py    # Full daily pipeline: fetch → analyze → embed
├── data/                         # Local data directory (gitignored)
│   ├── trikeri.db               # SQLite database
│   ├── chromadb/                # ChromaDB persistence directory
│   └── exports/                 # JSON/CSV exports
├── package.json
├── tsconfig.json
├── vite.config.ts
└── requirements.txt
```

### SQLite Data Models

```sql
-- Products / things we're promoting
CREATE TABLE products (
    id TEXT PRIMARY KEY,          -- uuid
    name TEXT NOT NULL,           -- "Trik_Klip", "4D Manifold Visualizer"
    type TEXT NOT NULL,           -- "paid_software", "free_tool", "interactive_page", "content"
    description TEXT,
    url TEXT,                     -- Primary link (Gumroad, webpage, etc.)
    price REAL,                   -- NULL for free products
    tags TEXT,                    -- JSON array: ["developer", "streaming", "premiere-pro"]
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- A campaign is a coordinated promotion push for a product
CREATE TABLE campaigns (
    id TEXT PRIMARY KEY,
    product_id TEXT NOT NULL REFERENCES products(id),
    name TEXT NOT NULL,           -- "Trik_Klip Launch Week", "Manifold Viz Reddit Push"
    status TEXT DEFAULT 'active', -- "active", "paused", "completed", "archived"
    goal TEXT,                    -- "drive_sales", "awareness", "traffic", "community_growth"
    target_audience TEXT,         -- Free text: "indie game devs, streamers, content creators"
    start_date DATE,
    end_date DATE,               -- NULL if ongoing
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Individual posts across any platform
CREATE TABLE posts (
    id TEXT PRIMARY KEY,
    campaign_id TEXT NOT NULL REFERENCES campaigns(id),
    platform TEXT NOT NULL,       -- "reddit", "x", "youtube", "discord", "producthunt",
                                 -- "hackernews", "tiktok", "instagram", "linkedin", "other"
    post_type TEXT NOT NULL,      -- "text", "image", "video_short", "video_long",
                                 -- "thread", "comment", "link", "self_promo"
    platform_post_id TEXT,       -- Native ID for API lookups (Reddit: t3_xxxxx, YT: video_id)
    url TEXT,                    -- Direct URL to the post
    title TEXT,                  -- Post title if applicable
    body_preview TEXT,           -- First 500 chars of the post body
    target_community TEXT,       -- "r/gamedev", "Discord: Indie Devs #self-promo", etc.
    posted_at TIMESTAMP,
    tags TEXT,                   -- JSON array for content tags
    is_api_tracked INTEGER DEFAULT 0,  -- 1 if we can auto-fetch metrics
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Daily metric snapshots per post (time-series, never overwritten)
CREATE TABLE metric_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id TEXT NOT NULL REFERENCES posts(id),
    snapshot_date DATE NOT NULL,
    views INTEGER DEFAULT 0,
    impressions INTEGER DEFAULT 0,
    likes INTEGER DEFAULT 0,          -- Upvotes on Reddit, likes on X/YT
    dislikes INTEGER DEFAULT 0,       -- Downvotes on Reddit, dislikes on YT
    comments INTEGER DEFAULT 0,
    shares INTEGER DEFAULT 0,         -- Retweets, crossposts, etc.
    saves INTEGER DEFAULT 0,          -- Reddit saves, YT "save to playlist"
    clicks INTEGER DEFAULT 0,         -- Link clicks if trackable
    watch_time_seconds INTEGER,       -- Video platforms only
    followers_gained INTEGER DEFAULT 0,
    custom_metrics TEXT,              -- JSON for platform-specific extras
    fetched_via TEXT DEFAULT 'manual', -- "api", "manual", "scrape"
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(post_id, snapshot_date)
);

-- AI analysis results (one per campaign per analysis run)
CREATE TABLE ai_analyses (
    id TEXT PRIMARY KEY,
    campaign_id TEXT REFERENCES campaigns(id),  -- NULL for cross-campaign analyses
    analysis_type TEXT NOT NULL,     -- "campaign_daily", "cross_campaign", "product_type_insight"
    summary TEXT NOT NULL,           -- Claude's high-level summary
    top_performers TEXT,             -- JSON: [{post_id, score, reasoning}]
    underperformers TEXT,            -- JSON: [{post_id, score, reasoning}]
    patterns TEXT,                   -- JSON: [{pattern, confidence, evidence}]
    recommendations TEXT,            -- JSON: [{action, priority, reasoning}]
    raw_response TEXT,               -- Full Claude response for debugging
    model_used TEXT,
    tokens_used INTEGER,
    analyzed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Tracks when major pipeline events last ran
CREATE TABLE system_state (
    key TEXT PRIMARY KEY,
    value TEXT,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
-- Keys: "last_metric_fetch", "last_ai_analysis", "last_embedding_update"

-- API credentials and per-platform config (encrypted at rest)
CREATE TABLE platform_configs (
    platform TEXT PRIMARY KEY,
    credentials TEXT,               -- JSON, encrypted: {client_id, client_secret, token, ...}
    is_enabled INTEGER DEFAULT 0,
    rate_limit_remaining INTEGER,
    last_fetched_at TIMESTAMP,
    config TEXT                     -- JSON: platform-specific settings
);
```

### Backend API Contract

```yaml
# ── Campaigns ──
GET    /api/campaigns                    → list all campaigns (with latest metrics summary)
GET    /api/campaigns/{id}               → campaign detail + posts + latest AI analysis
POST   /api/campaigns                    → create campaign
PUT    /api/campaigns/{id}               → update campaign
DELETE /api/campaigns/{id}               → archive campaign (soft delete)

# ── Products ──
GET    /api/products                     → list all products
POST   /api/products                     → create product
PUT    /api/products/{id}                → update product

# ── Posts ──
GET    /api/campaigns/{id}/posts         → posts for a campaign
POST   /api/campaigns/{id}/posts         → add post to campaign
PUT    /api/posts/{id}                   → update post
DELETE /api/posts/{id}                   → remove post

# ── Metrics ──
GET    /api/posts/{id}/metrics           → time-series metric snapshots for a post
GET    /api/campaigns/{id}/metrics       → aggregated metrics for all posts in campaign
POST   /api/metrics/fetch                → trigger manual metric fetch for all tracked posts
GET    /api/metrics/fetch/status         → check if a fetch is currently running

# ── Analytics (aggregated views) ──
GET    /api/analytics/overview           → dashboard summary: total campaigns, top posts, etc.
GET    /api/analytics/platforms          → engagement breakdown by platform
GET    /api/analytics/post-types         → engagement breakdown by post type
GET    /api/analytics/trends             → time-series engagement across all campaigns

# ── AI Analysis ──
GET    /api/ai/latest                    → most recent analysis for each active campaign
GET    /api/ai/campaign/{id}             → all analyses for a campaign
POST   /api/ai/trigger                   → manually trigger AI analysis now
GET    /api/ai/status                    → last run time, next scheduled, running state
GET    /api/ai/recommendations           → cross-campaign strategic recommendations
GET    /api/ai/knowledge-base/query      → semantic search the vector DB
GET    /api/ai/knowledge-base/stats      → vector DB size, coverage, last update

# ── Settings ──
GET    /api/settings                     → all settings (API keys redacted)
PUT    /api/settings/platform/{name}     → update platform credentials
PUT    /api/settings/general             → update general settings
GET    /api/settings/health              → check which platform APIs are connected & healthy

# ── System ──
GET    /api/system/startup-check         → returns what needs updating (metrics, AI analysis)
POST   /api/system/export/{campaign_id}  → export campaign data as JSON/CSV
```

### Platform Connector Interface

Every connector implements this interface so the metric collector can treat them uniformly:

```python
class PlatformConnector(ABC):
    platform: str

    @abstractmethod
    async def validate_credentials(self) -> bool:
        """Test if stored API credentials are valid."""

    @abstractmethod
    async def fetch_post_metrics(self, post: Post) -> MetricSnapshot:
        """Fetch current metrics for a single post."""

    @abstractmethod
    async def resolve_post_id(self, url: str) -> str | None:
        """Extract platform-native post ID from a URL."""

    @abstractmethod
    def is_api_trackable(self, post: Post) -> bool:
        """Whether this post can be auto-tracked via API."""
```

---

## Agent 2: Platform Connectors

**Responsibility:** Implement every platform's API integration. Each connector auto-fetches metrics and normalizes them into the shared `MetricSnapshot` schema.

### Reddit Connector (via PRAW / async PRAW)

```
Auth: OAuth2 "script" app (client_id + client_secret + username + password)
Rate limit: 100 requests / minute (very generous)
Free tier: Full access for personal scripts

Data available per post (submission):
  - score (upvotes - downvotes), upvote_ratio
  - num_comments
  - view_count (only if you're the author AND it's on your profile — unreliable)
  - num_crossposts
  - total_awards_received (less relevant now)
  - created_utc

Mapping to MetricSnapshot:
  likes      = score (net upvotes)
  comments   = num_comments
  shares     = num_crossposts
  saves      = NOT available via API
  views      = NOT reliably available (use impression estimates from upvote_ratio)
  clicks     = NOT available via API

Lookup strategy:
  - Store Reddit post ID as platform_post_id (e.g., "t3_abc123")
  - Or parse from URL: reddit.com/r/gamedev/comments/abc123/...
  - PRAW: reddit.submission(id="abc123")

Implementation notes:
  - Use asyncpraw for non-blocking fetches
  - Batch: iterate all Reddit posts, one submission lookup each
  - Comment sentiment: optionally pull top 5 comments for AI sentiment analysis
  - Respect subreddit-specific rules stored in post.target_community
```

### YouTube Connector (via YouTube Data API v3)

```
Auth: API key (simple) or OAuth2 (for channel-specific data)
Rate limit: 10,000 units/day free quota
Cost: ~5 units per videos.list call (very cheap)

Data available per video:
  - viewCount, likeCount, commentCount
  - favoriteCount (deprecated but still returned)
  - duration (useful for Shorts detection: ≤60s)
  - Statistics refresh frequently (near real-time)

Mapping to MetricSnapshot:
  views      = viewCount
  likes      = likeCount
  comments   = commentCount
  shares     = NOT directly available (no share count in API)
  watch_time = NOT available per-video without YouTube Analytics API (requires OAuth)

Lookup strategy:
  - Store video ID as platform_post_id (e.g., "dQw4w9WgXcQ")
  - Parse from URL: youtube.com/watch?v=ID or youtube.com/shorts/ID
  - API call: youtube.videos().list(part="statistics", id=VIDEO_ID)

Implementation notes:
  - YouTube Analytics API (separate from Data API) can give watch time,
    impressions, CTR — but requires OAuth and channel ownership verification
  - For MVP, use Data API v3 (API key only) for view/like/comment counts
  - Upgrade to Analytics API in a later phase for deeper metrics
  - Shorts and regular videos use the same API endpoint
```

### X / Twitter Connector (via X API v2)

```
Auth: OAuth 2.0 with PKCE (user context) or App-only (limited)
Rate limit: Free tier = 1 app, read-only, 1,500 tweets/month read
  - Basic tier ($100/month): 10,000 reads/month, more endpoints
  - For MVP: Free tier is likely sufficient for tracking <50 posts

Data available per tweet:
  - public_metrics: { retweet_count, reply_count, like_count, quote_count,
                      impression_count, bookmark_count }
  - Note: impression_count requires user auth context (OAuth 2.0 PKCE)

Mapping to MetricSnapshot:
  views       = impression_count (requires user auth)
  impressions = impression_count
  likes       = like_count
  comments    = reply_count
  shares      = retweet_count + quote_count
  saves       = bookmark_count

Lookup strategy:
  - Store tweet ID as platform_post_id (e.g., "1234567890")
  - Parse from URL: x.com/username/status/1234567890
  - API: GET /2/tweets/:id?tweet.fields=public_metrics

Implementation notes:
  - Free tier lacks impression_count — consider Basic if X is a primary channel
  - OAuth PKCE flow: Tauri can handle the redirect URI locally
  - Rate limits are tight on free — batch tweet IDs into single requests
    (up to 100 IDs per GET /2/tweets?ids=...)
  - Threads: store each tweet in thread separately, or store the first tweet
    and sum metrics across the thread via conversation_id lookup
```

### Discord Connector (hybrid: bot + manual)

```
REALITY CHECK: Discord has NO public analytics API for servers you don't own.
There is no way to programmatically fetch reaction counts, reply counts, or
view counts for messages in someone else's server.

Strategy — three tiers:

Tier 1: Manual entry (works everywhere, MVP)
  - Dashboard prompts: "Update Discord metrics for these posts"
  - User enters: reactions, replies, any DMs received
  - Lowest effort to build, highest effort to maintain

Tier 2: Personal bot in YOUR server (if you have one)
  - A small Discord bot (discord.py) that runs alongside the app
  - Can track reactions and replies on messages in servers where the bot is added
  - Limited to servers you control or that allow bot additions
  - Good for tracking engagement in your own community server

Tier 3: Webhook listener (future)
  - If posting via a bot, you can track reactions via gateway events
  - Requires the bot to be present in the target server

Mapping to MetricSnapshot:
  likes      = reaction_count (total across all emoji)
  comments   = reply_count (thread replies)
  views      = NOT available
  shares     = NOT available
  clicks     = NOT available (unless using tracked links)

Implementation notes:
  - For MVP, implement Tier 1 (manual) with a streamlined UI
  - Mark Discord posts as is_api_tracked = 0
  - If user has their own Discord server, offer Tier 2 as optional setup
  - Store server name + channel in target_community field
```

### Manual / Other Platform Connector

```
For platforms without API access:
  - Product Hunt, Hacker News, Instagram, LinkedIn, TikTok (limited API)
  - Provides a quick-entry UI form for each metric field
  - Can optionally provide a URL that the app opens in a browser
    so the user can eyeball the numbers and type them in

Hacker News special case:
  - HN has an open API (no auth needed): https://hacker-news.firebaseio.com/
  - GET /v0/item/{id}.json returns: score, descendants (comment count)
  - Can auto-track! Parse ID from URL: news.ycombinator.com/item?id=12345
  - Mark as is_api_tracked = 1

TikTok note:
  - TikTok API requires app review and is painful
  - For MVP, treat as manual entry
  - Consider TikTok Research API if volume justifies it later
```

### Metric Collection Orchestrator

```python
# backend/services/metric_collector.py

class MetricCollector:
    """
    Runs on app startup (if stale) and can be manually triggered.
    Iterates all posts with is_api_tracked=1, fetches via the appropriate
    connector, writes MetricSnapshot rows.
    """

    async def collect_all(self) -> CollectionReport:
        """
        1. Load all posts where is_api_tracked = 1
        2. Group by platform
        3. For each platform, load the connector + credentials
        4. Fetch metrics for each post (batch where possible)
        5. Write MetricSnapshot rows (UPSERT on post_id + snapshot_date)
        6. Return summary: {fetched: 42, failed: 2, skipped: 5, manual_needed: 8}
        """

    async def collect_campaign(self, campaign_id: str) -> CollectionReport:
        """Same as above but scoped to one campaign's posts."""

    async def collect_post(self, post_id: str) -> MetricSnapshot:
        """Fetch metrics for a single post on demand."""
```

---

## Agent 3: Backend Core

**Responsibility:** FastAPI application, SQLite database layer, sidecar configuration, and the scheduling/staleness system.

### Sidecar Architecture

```
Tauri launches Python as a sidecar process:

  [Tauri Window] ←→ [localhost:31415/api/*] ←→ [SQLite + ChromaDB]
       ↑                     ↑
    React UI           Python FastAPI
  (Vite dev or         (bundled with
   built static)        PyInstaller)

Startup sequence:
  1. Tauri's main.rs spawns the Python sidecar
  2. Python starts FastAPI on a random available port (or fixed 31415)
  3. Python writes port to a temp file; Tauri reads it
  4. React frontend uses that port for all API calls
  5. On app close, Tauri sends SIGTERM to the Python process

For development:
  - Run `uvicorn backend.main:app --port 31415 --reload` manually
  - Run `npm run tauri dev` or just `vite dev` for the frontend
  - Both connect via localhost

For production build:
  - PyInstaller bundles the Python backend into a single executable
  - Tauri bundles that executable as a sidecar resource
  - Tauri.conf.json references it in the "sidecar" config
```

### Startup Staleness Check

```python
# backend/main.py — on startup

@app.on_event("startup")
async def startup_check():
    """
    Called when FastAPI starts (= when user opens the app).
    Checks what's stale and triggers background tasks.
    """
    state = get_system_state()  # reads from system_state table

    now = datetime.utcnow()

    # 1. Metric fetch: if >6 hours stale, auto-fetch
    last_fetch = state.get("last_metric_fetch")
    if not last_fetch or (now - last_fetch) > timedelta(hours=6):
        background_tasks.add_task(metric_collector.collect_all)
        # Update the state immediately to prevent duplicate triggers
        set_system_state("last_metric_fetch", now.isoformat())

    # 2. AI analysis: if >24 hours stale, auto-trigger
    last_analysis = state.get("last_ai_analysis")
    if not last_analysis or (now - last_analysis) > timedelta(hours=24):
        # Wait for metrics to finish first, then run analysis
        background_tasks.add_task(daily_pipeline.run_analysis_after_fetch)
        set_system_state("last_ai_analysis", now.isoformat())

# The frontend also exposes the staleness info via:
# GET /api/system/startup-check
# → { metrics_stale: true, metrics_last_run: "...",
#     analysis_stale: true, analysis_last_run: "..." }
# The UI shows a subtle refresh indicator while background tasks run
```

### Recurring Check While App Is Open

```python
# backend/ai/scheduler.py

class AnalysisScheduler:
    """
    While the app is running, check every hour if 24h has passed
    since last AI analysis. This handles the case where someone
    leaves the app open for days.
    """

    def __init__(self):
        self._task: asyncio.Task | None = None

    async def start(self):
        self._task = asyncio.create_task(self._loop())

    async def _loop(self):
        while True:
            await asyncio.sleep(3600)  # Check every hour
            last = get_system_state("last_ai_analysis")
            if not last or (utcnow() - parse(last)) > timedelta(hours=24):
                await daily_pipeline.run_full()
                set_system_state("last_ai_analysis", utcnow().isoformat())

    async def stop(self):
        if self._task:
            self._task.cancel()
```

### Database Layer

```
- SQLAlchemy 2.0 with async support (aiosqlite)
- Alembic for schema migrations (future-proofing)
- Connection pool: single file, WAL mode for concurrent reads
- All timestamps stored as UTC ISO strings
- JSON fields stored as TEXT, validated with Pydantic on read/write
- Full-text search on posts (title, body_preview) via SQLite FTS5
```

---

## Agent 4: AI Brain

**Responsibility:** Claude API integration, analysis pipeline, ChromaDB vector storage, and the learning loop.

### Analysis Pipeline

```
Daily pipeline flow:

  1. COLLECT   → Agent 2's connectors fetch fresh metrics
  2. PREPARE   → Build analysis context per campaign
  3. ANALYZE   → Send to Claude API, get structured response
  4. STORE     → Save analysis to SQLite
  5. EMBED     → Generate embeddings, store in ChromaDB
  6. RECOMMEND → Query ChromaDB for new campaign suggestions

Step 2 — Context Preparation:
  For each active campaign, build a rich context document:
  {
    "campaign": { name, product, type, goal, audience, age_days },
    "posts": [
      {
        "platform": "reddit",
        "post_type": "image",
        "target": "r/gamedev",
        "age_days": 5,
        "metrics_trajectory": [
          { "day": 1, "views": 0, "likes": 23, "comments": 7 },
          { "day": 2, "views": 0, "likes": 45, "comments": 12 },
          { "day": 5, "views": 0, "likes": 52, "comments": 14 }
        ],
        "engagement_velocity": "fast_start_plateau",
        "body_preview": "First 200 chars of post..."
      },
      ...
    ],
    "historical_context": [  // From ChromaDB
      "Similar past campaign for paid dev tool on Reddit: GIF posts
       outperformed text by 3.2x...",
      ...
    ]
  }
```

### Claude API Prompt Structure

```python
# backend/ai/prompts.py

CAMPAIGN_ANALYSIS_SYSTEM = """
You are the AI marketing analyst for Trikeri, an independent creator brand
that builds tools, software, and interactive experiences. You analyze
campaign performance data and provide actionable strategic insights.

Respond ONLY in valid JSON matching this schema:
{
  "summary": "2-3 sentence overview of campaign health",
  "effectiveness_score": 0-100,
  "top_performers": [
    {
      "post_id": "...",
      "score": 0-100,
      "reasoning": "Why this post worked"
    }
  ],
  "underperformers": [
    {
      "post_id": "...",
      "score": 0-100,
      "reasoning": "Why this post underperformed and what to change"
    }
  ],
  "patterns": [
    {
      "pattern": "Description of the pattern",
      "confidence": "high|medium|low",
      "evidence": "Specific data points supporting this",
      "actionable_insight": "What to do about it"
    }
  ],
  "recommendations": [
    {
      "action": "Specific action to take",
      "priority": "high|medium|low",
      "reasoning": "Why this would help",
      "estimated_impact": "What improvement to expect"
    }
  ],
  "meta_learning": {
    "product_type_insight": "What this campaign teaches about promoting this TYPE of product",
    "platform_insight": "Platform-specific learnings",
    "audience_insight": "What we learned about the target audience",
    "content_format_insight": "What content formats worked and why"
  }
}
"""

CROSS_CAMPAIGN_SYSTEM = """
You are analyzing ALL campaigns across the Trikeri brand to find
cross-cutting patterns. Look for:
- Which product types benefit most from which platforms
- Content format effectiveness across different audiences
- Timing patterns (day of week, time of day if available)
- Community-specific behaviors (which subreddits, Discord servers respond best)
- Price sensitivity signals (do free products promote differently than paid?)

Same JSON schema as campaign analysis, but patterns and recommendations
should be cross-campaign strategic insights.
"""
```

### ChromaDB Vector Knowledge Base

```python
# backend/ai/embedder.py

COLLECTION_NAME = "trikeri_marketing_knowledge"

# What gets embedded (each is a document in the collection):

# 1. Campaign completion summaries
#    Embedded when a campaign is marked "completed"
#    Document: Rich narrative combining product info + strategy + results + AI analysis
#    Metadata: { product_type, platforms_used, post_types, effectiveness_score,
#                campaign_duration_days, total_engagement }

# 2. High-signal post analyses
#    Embedded for any post scoring >75 or <25 in AI analysis (strong signal)
#    Document: Post details + trajectory + AI reasoning about why it worked/failed
#    Metadata: { platform, post_type, target_community, engagement_velocity,
#                product_type, score }

# 3. Pattern observations
#    Embedded whenever AI identifies a high-confidence pattern
#    Document: The pattern description + evidence + actionable insight
#    Metadata: { confidence, pattern_type, platforms_involved, product_types }

# 4. Weekly cross-campaign digests
#    Embedded every 7 days as a rolling strategic summary
#    Document: Full cross-campaign analysis with trends over time
#    Metadata: { week_start, week_end, num_campaigns_active, total_posts }


class MarketingKnowledgeBase:
    def __init__(self):
        self.client = chromadb.PersistentClient(path="data/chromadb")
        self.collection = self.client.get_or_create_collection(
            name=COLLECTION_NAME,
            embedding_function=default_ef()  # Uses all-MiniLM-L6-v2 by default
        )

    async def query_similar_campaigns(
        self, product_type: str, platforms: list[str], goal: str, n: int = 5
    ) -> list[dict]:
        """
        When starting a new campaign, query for historically similar ones.
        Returns the most relevant past learnings.
        """
        query_text = (
            f"Marketing campaign for {product_type} product, "
            f"promoting on {', '.join(platforms)}, goal: {goal}"
        )
        results = self.collection.query(
            query_texts=[query_text],
            n_results=n,
            where={"product_type": {"$in": [product_type, "any"]}}
        )
        return results

    async def embed_campaign_completion(self, campaign_id: str):
        """Build and store the rich completion document."""
        # Pulls campaign + all posts + all metrics + all AI analyses
        # Constructs a narrative document
        # Embeds with metadata for future retrieval

    async def embed_post_insight(self, post_id: str, analysis: dict):
        """Store a strong-signal post learning."""

    async def embed_pattern(self, pattern: dict, analysis_id: str):
        """Store a high-confidence pattern observation."""

    async def get_stats(self) -> dict:
        """Return collection size, coverage info."""
        return {
            "total_documents": self.collection.count(),
            "coverage": {
                "campaigns_embedded": ...,
                "posts_embedded": ...,
                "patterns_stored": ...
            }
        }
```

### Full Daily Pipeline

```python
# backend/services/daily_pipeline.py

class DailyPipeline:
    """
    The full daily cycle:
    1. Fetch all metrics (via connectors)
    2. Run AI analysis on each active campaign
    3. Run cross-campaign analysis
    4. Embed new learnings into ChromaDB
    5. Update system state timestamps
    """

    async def run_full(self):
        report = {"started_at": utcnow(), "steps": []}

        # Step 1: Fetch metrics
        fetch_result = await self.metric_collector.collect_all()
        report["steps"].append({"fetch": fetch_result})

        # Step 2: Per-campaign AI analysis
        active_campaigns = await self.db.get_active_campaigns()
        for campaign in active_campaigns:
            context = await self.build_campaign_context(campaign)
            analysis = await self.analyzer.analyze_campaign(context)
            await self.db.save_analysis(analysis)
            report["steps"].append({"campaign_analysis": campaign.id})

            # Step 4a: Embed high-signal posts
            for post in analysis.top_performers + analysis.underperformers:
                if post.score > 75 or post.score < 25:
                    await self.knowledge_base.embed_post_insight(post.post_id, post)

            # Step 4b: Embed high-confidence patterns
            for pattern in analysis.patterns:
                if pattern.confidence == "high":
                    await self.knowledge_base.embed_pattern(pattern, analysis.id)

        # Step 3: Cross-campaign analysis
        if len(active_campaigns) > 1:
            cross_analysis = await self.analyzer.analyze_cross_campaign(active_campaigns)
            await self.db.save_analysis(cross_analysis)
            report["steps"].append({"cross_analysis": True})

        # Step 5: Update timestamps
        set_system_state("last_ai_analysis", utcnow().isoformat())
        set_system_state("last_metric_fetch", utcnow().isoformat())

        return report

    async def run_analysis_after_fetch(self):
        """Wait for any in-progress fetch, then run analysis."""
        while self.metric_collector.is_running:
            await asyncio.sleep(5)
        await self.run_full()
```

---

## Agent 5: Frontend Dashboard

**Responsibility:** Full React UI with Recharts/Nivo visualizations, campaign management flows, and the AI insights panel.

### Page Structure

```
App Layout:
┌─────────────────────────────────────────────────┐
│  [Logo] Trikeri Marketing Engine     [⚙ Settings] │
├──────────┬──────────────────────────────────────┤
│          │                                      │
│ Sidebar  │  Main Content Area                   │
│          │                                      │
│ Dashboard│  (varies by route)                   │
│ Campaigns│                                      │
│ Analytics│                                      │
│ AI Brain │                                      │
│          │                                      │
│          │                                      │
│ ──────── │                                      │
│ Status:  │                                      │
│ ● Metrics│                                      │
│   fresh  │                                      │
│ ○ AI     │                                      │
│   stale  │                                      │
└──────────┴──────────────────────────────────────┘
```

### Dashboard Page (/)

```
Overview cards at top:
  [Active Campaigns: 3]  [Posts Tracked: 47]  [Avg Engagement: 4.2%]  [AI Score: 72]

Campaign cards grid:
  Each card shows:
    - Campaign name + product name
    - Status badge (active/paused/completed)
    - Platform icons showing where posts exist
    - Mini sparkline of total engagement over last 14 days
    - AI effectiveness score (color-coded: green >70, amber 40-70, red <40)
    - "Last updated: 2h ago"

Bottom section:
  "AI says:" — Latest cross-campaign insight in a callout box
  "Quick actions:" — [+ New Campaign] [Fetch Metrics Now] [Run AI Analysis]
```

### Campaign Detail Page (/campaigns/:id)

```
Header:
  Campaign name | Product | Status | Date range
  AI effectiveness score (large, color-coded)

Tab bar: [Posts] [Metrics] [AI Insights] [Settings]

Posts tab:
  Table/list of all posts:
    Platform icon | Title/Preview | Community | Posted date | Engagement summary
    Each row expandable to show metric trajectory sparkline
    [+ Add Post] button with smart URL parser:
      - Paste a Reddit/YouTube/X URL
      - Auto-detects platform and post ID
      - Pre-fills platform, post_type, target_community, platform_post_id
      - Marks is_api_tracked = 1 if API connector exists

Metrics tab:
  Time-series chart: engagement across all posts over time
  Breakdown by platform (stacked area or grouped bar)
  Breakdown by post type
  Heatmap: day-of-week × time engagement patterns (if data available)

AI Insights tab:
  Latest AI analysis summary
  Top performers with reasoning (expandable cards)
  Underperformers with suggestions
  Detected patterns (confidence-tagged)
  Recommendations (priority-sorted, actionable)
  "Similar past campaigns:" — results from ChromaDB semantic query

Settings tab:
  Edit campaign name, goal, audience
  Archive campaign (triggers completion embedding)
```

### AI Brain Page (/ai)

```
Two sections:

1. Latest Intelligence:
   - Per-campaign AI summaries (cards, latest analysis for each)
   - Cross-campaign strategic insights
   - Pattern library: all high-confidence patterns detected, sortable

2. Knowledge Base Explorer:
   - Search bar: semantic query against ChromaDB
     "What works for promoting free developer tools on Reddit?"
   - Results shown as cards with relevance score
   - Stats: total documents, campaigns covered, last update
   - Visual: simple chart showing knowledge growth over time
```

### Settings Page (/settings)

```
Sections:

1. Platform Connections
   For each platform:
     [Reddit]     ● Connected    [Test] [Edit Credentials]
     [YouTube]    ● Connected    [Test] [Edit Credentials]
     [X/Twitter]  ○ Not configured  [Setup]
     [Discord]    ○ Manual only     [Setup Bot]
     [HN]         ● Auto (no auth)

2. AI Configuration
   - Claude API key: [••••••••••] [Edit]
   - Analysis model: claude-sonnet-4-20250514 (dropdown)
   - Auto-analysis: ● Enabled (every 24h)
   - Embedding model: all-MiniLM-L6-v2 (local, no API needed)

3. Appearance
   - Theme selector: visual swatches showing each color scheme
     ● Peach Sunset (default)
     ○ Midnight
     ○ Forest
     ○ Ocean
   - Compact mode toggle (smaller cards, denser tables)

4. Data
   - Export all data as JSON
   - Export campaign as CSV
   - Database location: C:\Users\Troy\AppData\...
   - Reset AI knowledge base (with confirmation)
```

---

## Agent 6: Packaging & Config

**Responsibility:** Tauri shell configuration, Python sidecar bundling, theme system, and Windows build pipeline.

### Tauri Configuration

```json
// src-tauri/tauri.conf.json (key sections)
{
  "productName": "Trikeri Marketing Engine",
  "identifier": "com.trikeri.marketing-engine",
  "build": {
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "bundle": {
    "active": true,
    "targets": ["nsis"],
    "windows": {
      "nsis": {
        "installMode": "both"
      }
    },
    "externalBin": ["binaries/marketing-engine-backend"]
  },
  "app": {
    "windows": [
      {
        "title": "Trikeri Marketing Engine",
        "width": 1400,
        "height": 900,
        "minWidth": 1000,
        "minHeight": 700,
        "decorations": true,
        "resizable": true
      }
    ]
  }
}
```

### Python Sidecar Bundling

```
Build pipeline:

1. PyInstaller bundles the entire Python backend:
   pyinstaller --onefile --name marketing-engine-backend backend/main.py

2. The resulting .exe goes into src-tauri/binaries/
   Naming convention: marketing-engine-backend-x86_64-pc-windows-msvc.exe
   (Tauri requires the target triple suffix)

3. Tauri's externalBin config references "binaries/marketing-engine-backend"
   and auto-appends the platform suffix

4. At runtime, Tauri spawns the sidecar:
   - Rust: Command::new_sidecar("marketing-engine-backend").spawn()
   - The Python process starts FastAPI, binds to a port
   - Port negotiation: Python writes to stdout, Tauri reads it

Dependencies to bundle:
  - fastapi, uvicorn, aiosqlite, sqlalchemy
  - praw (asyncpraw), google-api-python-client
  - tweepy or httpx (for X API)
  - chromadb, sentence-transformers (for local embeddings)
  - anthropic (Claude SDK)
  - Note: sentence-transformers + torch is ~2GB
    → Consider using chromadb's built-in default embedding (smaller)
    → Or use Anthropic's embedding API to avoid bundling torch entirely
```

### Color Theme System

```json
// src/theme/themes.json
{
  "peach_sunset": {
    "id": "peach_sunset",
    "name": "Peach Sunset",
    "description": "Default warm palette",
    "colors": {
      "bg_primary": "#FFF8F2",
      "bg_secondary": "#FFF1E6",
      "bg_tertiary": "#FFE8D6",
      "bg_elevated": "#FFFFFF",

      "surface_card": "#FFFFFF",
      "surface_sidebar": "#FFF1E6",
      "surface_hover": "#FFE0CC",

      "text_primary": "#3D2B1F",
      "text_secondary": "#7A5C48",
      "text_tertiary": "#A68B7B",
      "text_on_accent": "#FFFFFF",

      "accent_primary": "#E8845C",
      "accent_primary_hover": "#D6733E",
      "accent_secondary": "#F4A67B",
      "accent_tertiary": "#FFD4B8",

      "salmon": "#FA8072",
      "salmon_light": "#FFA69E",
      "salmon_bg": "#FFF0EE",

      "creamsicle": "#FFB347",
      "creamsicle_light": "#FFCC80",
      "creamsicle_bg": "#FFF5E6",

      "cyan": "#4DD0E1",
      "cyan_light": "#80DEEA",
      "cyan_dark": "#00ACC1",
      "cyan_bg": "#E0F7FA",

      "peach": "#FFDAB9",
      "peach_dark": "#E8A87C",

      "success": "#66BB6A",
      "success_bg": "#F1F8F1",
      "warning": "#FFB74D",
      "warning_bg": "#FFF8E1",
      "danger": "#EF5350",
      "danger_bg": "#FFF0F0",
      "info": "#4DD0E1",
      "info_bg": "#E0F7FA",

      "border_light": "rgba(232, 132, 92, 0.12)",
      "border_medium": "rgba(232, 132, 92, 0.24)",
      "border_strong": "rgba(232, 132, 92, 0.40)",

      "chart_1": "#E8845C",
      "chart_2": "#4DD0E1",
      "chart_3": "#FA8072",
      "chart_4": "#FFB347",
      "chart_5": "#66BB6A",
      "chart_6": "#AB47BC",

      "sparkline_up": "#66BB6A",
      "sparkline_down": "#EF5350",
      "sparkline_neutral": "#FFB347",

      "score_high": "#66BB6A",
      "score_mid": "#FFB347",
      "score_low": "#EF5350",

      "shadow_sm": "0 1px 3px rgba(61, 43, 31, 0.06)",
      "shadow_md": "0 4px 12px rgba(61, 43, 31, 0.08)",
      "shadow_lg": "0 8px 24px rgba(61, 43, 31, 0.10)",

      "radius_sm": "6px",
      "radius_md": "10px",
      "radius_lg": "16px",
      "radius_xl": "24px"
    }
  },

  "midnight": {
    "id": "midnight",
    "name": "Midnight",
    "description": "Dark mode with peach accents",
    "colors": {
      "bg_primary": "#1A1A2E",
      "bg_secondary": "#16213E",
      "bg_tertiary": "#0F3460",
      "bg_elevated": "#222244",

      "surface_card": "#222244",
      "surface_sidebar": "#16213E",
      "surface_hover": "#2A2A4A",

      "text_primary": "#F5F0EB",
      "text_secondary": "#B8A99A",
      "text_tertiary": "#7A6E63",
      "text_on_accent": "#1A1A2E",

      "accent_primary": "#E8845C",
      "accent_primary_hover": "#F4A67B",
      "accent_secondary": "#F4A67B",
      "accent_tertiary": "#FFD4B8",

      "salmon": "#FA8072",
      "salmon_light": "#FFA69E",
      "salmon_bg": "#2E1A1A",

      "creamsicle": "#FFB347",
      "creamsicle_light": "#FFCC80",
      "creamsicle_bg": "#2E2410",

      "cyan": "#4DD0E1",
      "cyan_light": "#80DEEA",
      "cyan_dark": "#00ACC1",
      "cyan_bg": "#0A2E33",

      "peach": "#FFDAB9",
      "peach_dark": "#E8A87C",

      "success": "#81C784",
      "success_bg": "#1A2E1A",
      "warning": "#FFB74D",
      "warning_bg": "#2E2410",
      "danger": "#EF5350",
      "danger_bg": "#2E1A1A",
      "info": "#4DD0E1",
      "info_bg": "#0A2E33",

      "border_light": "rgba(232, 132, 92, 0.10)",
      "border_medium": "rgba(232, 132, 92, 0.20)",
      "border_strong": "rgba(232, 132, 92, 0.35)",

      "chart_1": "#E8845C",
      "chart_2": "#4DD0E1",
      "chart_3": "#FA8072",
      "chart_4": "#FFB347",
      "chart_5": "#81C784",
      "chart_6": "#CE93D8",

      "sparkline_up": "#81C784",
      "sparkline_down": "#EF5350",
      "sparkline_neutral": "#FFB347",

      "score_high": "#81C784",
      "score_mid": "#FFB347",
      "score_low": "#EF5350",

      "shadow_sm": "0 1px 3px rgba(0, 0, 0, 0.20)",
      "shadow_md": "0 4px 12px rgba(0, 0, 0, 0.30)",
      "shadow_lg": "0 8px 24px rgba(0, 0, 0, 0.40)",

      "radius_sm": "6px",
      "radius_md": "10px",
      "radius_lg": "16px",
      "radius_xl": "24px"
    }
  },

  "ocean": {
    "id": "ocean",
    "name": "Ocean",
    "description": "Cool blues with warm accents",
    "colors": {
      "bg_primary": "#F0F8FF",
      "bg_secondary": "#E3F2FD",
      "bg_tertiary": "#BBDEFB",
      "bg_elevated": "#FFFFFF",

      "surface_card": "#FFFFFF",
      "surface_sidebar": "#E3F2FD",
      "surface_hover": "#D0EAFF",

      "text_primary": "#1A2332",
      "text_secondary": "#4A6276",
      "text_tertiary": "#7A97AD",
      "text_on_accent": "#FFFFFF",

      "accent_primary": "#0288D1",
      "accent_primary_hover": "#0277BD",
      "accent_secondary": "#4FC3F7",
      "accent_tertiary": "#B3E5FC",

      "salmon": "#F4796B",
      "salmon_light": "#FFA69E",
      "salmon_bg": "#FFF0EE",

      "creamsicle": "#FFB347",
      "creamsicle_light": "#FFCC80",
      "creamsicle_bg": "#FFF5E6",

      "cyan": "#00BCD4",
      "cyan_light": "#4DD0E1",
      "cyan_dark": "#00838F",
      "cyan_bg": "#E0F7FA",

      "peach": "#FFCCBC",
      "peach_dark": "#E8A87C",

      "success": "#66BB6A",
      "success_bg": "#F1F8F1",
      "warning": "#FFA726",
      "warning_bg": "#FFF8E1",
      "danger": "#EF5350",
      "danger_bg": "#FFF0F0",
      "info": "#29B6F6",
      "info_bg": "#E3F2FD",

      "border_light": "rgba(2, 136, 209, 0.10)",
      "border_medium": "rgba(2, 136, 209, 0.20)",
      "border_strong": "rgba(2, 136, 209, 0.35)",

      "chart_1": "#0288D1",
      "chart_2": "#F4796B",
      "chart_3": "#FFB347",
      "chart_4": "#66BB6A",
      "chart_5": "#AB47BC",
      "chart_6": "#4DD0E1",

      "sparkline_up": "#66BB6A",
      "sparkline_down": "#EF5350",
      "sparkline_neutral": "#FFA726",

      "score_high": "#66BB6A",
      "score_mid": "#FFA726",
      "score_low": "#EF5350",

      "shadow_sm": "0 1px 3px rgba(26, 35, 50, 0.06)",
      "shadow_md": "0 4px 12px rgba(26, 35, 50, 0.08)",
      "shadow_lg": "0 8px 24px rgba(26, 35, 50, 0.10)",

      "radius_sm": "6px",
      "radius_md": "10px",
      "radius_lg": "16px",
      "radius_xl": "24px"
    }
  }
}
```

### Theme Provider (React)

```tsx
// src/theme/ThemeProvider.tsx — high-level pattern

// 1. Load themes.json at build time (import themes from './themes.json')
// 2. Store selected theme ID in localStorage + SQLite settings
// 3. On mount, read theme ID, apply all colors as CSS custom properties
//    on document.documentElement
// 4. Every component uses var(--bg-primary), var(--text-primary), etc.
// 5. ThemeSwitcher component renders color swatches, updates on click

// CSS variable injection:
function applyTheme(themeId: string) {
  const theme = themes[themeId];
  const root = document.documentElement;
  Object.entries(theme.colors).forEach(([key, value]) => {
    root.style.setProperty(`--${key.replace(/_/g, '-')}`, value);
  });
  localStorage.setItem('trikeri-theme', themeId);
}
```

---

## Build Phases & Task Breakdown

### Phase 1: Foundation (Agent 1 + Agent 3) — Days 1-3

```
□ Initialize Tauri + Vite + React project
□ Set up Python backend with FastAPI
□ Create SQLite schema (all tables from data model above)
□ Implement database connection manager with WAL mode
□ Build CRUD endpoints: products, campaigns, posts
□ Implement system_state table + startup check logic
□ Verify sidecar communication (Tauri → Python → response)
□ Set up theme system (themes.json + ThemeProvider + CSS vars)
```

### Phase 2: Connectors + UI Shell (Agent 2 + Agent 5) — Days 4-8

```
Agent 2 (parallel):
  □ Implement base PlatformConnector interface
  □ Reddit connector (asyncpraw)
  □ YouTube connector (google-api-python-client)
  □ X/Twitter connector (httpx + OAuth2)
  □ Hacker News connector (open API, no auth)
  □ Manual entry connector (structured fallback)
  □ MetricCollector orchestrator
  □ URL parser: auto-detect platform + post ID from any URL

Agent 5 (parallel):
  □ App layout: sidebar, header, routing
  □ Dashboard page: campaign cards, overview metrics
  □ Campaign detail page: posts list, add post flow
  □ Smart URL paste: detect platform, pre-fill fields
  □ Metric charts: Recharts time-series per campaign
  □ Platform breakdown charts
  □ Settings page: API key forms, theme switcher
  □ Loading states: skeleton screens while fetching
  □ Status indicators: metric freshness, AI last-run
```

### Phase 3: AI Integration (Agent 4) — Days 9-12

```
□ Claude API integration with structured JSON responses
□ Campaign analysis prompt + response parser
□ Cross-campaign analysis prompt
□ ChromaDB setup with persistent storage
□ Embedding pipeline: campaign completions, post insights, patterns
□ Semantic query endpoint for knowledge base
□ Daily pipeline orchestrator (fetch → analyze → embed)
□ 24-hour staleness trigger on startup
□ Hourly background check while app is open
□ AI Insights page: summaries, recommendations, pattern library
□ Knowledge Base Explorer: semantic search UI
```

### Phase 4: Polish & Package (Agent 6) — Days 13-15

```
□ PyInstaller build for Python backend
□ Tauri sidecar configuration
□ Windows NSIS installer build
□ Error handling: what happens when APIs fail, keys are missing
□ Offline mode: graceful degradation when no internet
□ Data export: JSON + CSV per campaign
□ Onboarding flow: first launch wizard for API keys
□ Performance: lazy load charts, paginate post lists
□ Final theme polish: all components using CSS vars consistently
```

---

## Key Design Decisions & Rationale

**Why Tauri over Electron?**
Smaller binary (~10MB vs ~150MB), native Rust performance, better memory usage. Troy is already distributing Trik_Klip — same ecosystem, consistent brand.

**Why SQLite over Postgres?**
Local-first, zero config, single file backup, perfect for a desktop app. WAL mode handles concurrent reads from the UI while the background pipeline writes. No server to maintain.

**Why ChromaDB over Pinecone/Weaviate?**
Runs locally, no API costs, no cloud dependency. Uses sentence-transformers for local embedding — the entire knowledge base lives on Troy's machine. For a solo creator's marketing data, the scale is perfect (hundreds to low thousands of documents).

**Why separate Python sidecar instead of Rust backend?**
Python has the best ecosystem for the AI/ML stack: anthropic SDK, chromadb, asyncpraw, google-api-python-client, sentence-transformers. Writing connectors in Rust would triple development time for no user-facing benefit.

**Why daily snapshots instead of real-time metrics?**
Most platform APIs rate-limit aggressively. Social media engagement follows predictable curves (fast start, long tail). Daily snapshots capture the meaningful shape without burning API quota. The trajectory pattern (fast_start vs slow_burn vs viral) is more valuable than minute-by-minute numbers.

**Why embed AI reasoning, not just metrics?**
Raw numbers don't transfer across campaigns. "47 upvotes on r/gamedev" means nothing for the next campaign. But "for paid developer tools, Reddit posts with animated GIFs showing the tool in action outperform text posts by 3x on game-dev subreddits" — that transfers. The vector DB stores the *reasoning*, which is what makes it useful for future campaign planning.

**Embedding model choice:**
ChromaDB's default (all-MiniLM-L6-v2) runs locally, is ~80MB, and is good enough for semantic similarity on marketing text. Avoids bundling PyTorch (~2GB). If embedding quality matters more later, switch to Anthropic's embedding API (requires internet but tiny binary).
