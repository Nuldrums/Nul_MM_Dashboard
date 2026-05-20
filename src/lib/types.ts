export interface Profile {
  id: string;
  name: string;
  description?: string;
  avatar_color: string;
  created_at: string;
}

export interface Product {
  id: string;
  name: string;
  type: string; // "paid_software" | "free_tool" | "interactive_page" | "content"
  description?: string;
  url?: string;
  price?: number;
  tags: string[];
  created_at: string;
}

export interface Campaign {
  id: string;
  product_id: string;
  profile_id?: string;
  name: string;
  status: "active" | "paused" | "completed" | "archived";
  goal?: string;
  target_audience?: string[];
  tags?: string[];
  start_date?: string;
  end_date?: string;
  notes?: string;
  created_at: string;
  updated_at: string;
  product?: Product;
  posts?: Post[];
  latest_analysis?: AIAnalysis;
  metrics_summary?: MetricsSummary;
  post_count?: number;
  total_likes?: number;
  total_comments?: number;
  total_views?: number;
  platforms?: string[];
}

export interface Post {
  id: string;
  campaign_id: string;
  platform: string;
  post_type: string;
  platform_post_id?: string;
  url?: string;
  title?: string;
  body_preview?: string;
  target_community?: string;
  posted_at?: string;
  tags: string[];
  is_api_tracked: boolean;
  created_at: string;
  views?: number;
  likes?: number;
  comments?: number;
  shares?: number;
  engagement?: number;
}

export interface MetricSnapshot {
  id: number;
  post_id: string;
  snapshot_date: string;
  views: number;
  impressions: number;
  likes: number;
  dislikes: number;
  comments: number;
  shares: number;
  saves: number;
  clicks: number;
  watch_time_seconds?: number;
  followers_gained: number;
  custom_metrics?: Record<string, any>;
  fetched_via: string;
  created_at: string;
}

export interface AIAnalysis {
  id: string;
  campaign_id?: string;
  analysis_type: string;
  summary: string;
  top_performers?: { post_id: string; score: number; reasoning: string }[];
  underperformers?: { post_id: string; score: number; reasoning: string }[];
  patterns?: { pattern: string; confidence: string; evidence: string; actionable_insight?: string }[];
  recommendations?: { action: string; priority: string; reasoning: string; estimated_impact?: string }[];
  model_used?: string;
  tokens_used?: number;
  analyzed_at: string;
}

export interface MetricsSummary {
  total_views: number;
  total_likes: number;
  total_comments: number;
  total_shares: number;
  total_posts: number;
  avg_engagement: number;
  ai_score?: number;
}

export interface PlatformConfig {
  platform: string;
  is_enabled: boolean;
  last_fetched_at?: string;
}

export interface SystemStatus {
  metrics_stale: boolean;
  metrics_last_run?: string;
  analysis_stale: boolean;
  analysis_last_run?: string;
}

export type Platform = "reddit" | "x" | "youtube" | "discord" | "tiktok" | "instagram" | "linkedin" | "other";
export type PostType = "text" | "image" | "video_short" | "video_long" | "thread" | "comment" | "link" | "self_promo";

export interface CampaignFeed {
  id: string;
  campaign_id: string;
  profile_account_id: string;
  platform: string;
  account_handle: string;
  account_id?: string;
  follower_count?: number;
  follower_count_at?: string;
  content_type: string;
  last_seen_post_id?: string;
  last_checked_at?: string;
  last_error?: string;
  is_active: number;
  created_at?: string;
}

export interface ProfileAccount {
  id: string;
  profile_id: string;
  platform: string;
  account_handle: string;
  account_id?: string;
  is_active: number;
  has_oauth: number;
  token_expires_at?: string;
  created_at?: string;
}
