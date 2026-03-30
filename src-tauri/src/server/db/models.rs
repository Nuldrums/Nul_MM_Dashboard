use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize, Deserializer, Serializer};

// Custom serde for tags: stored as JSON string in DB, exposed as array in API

pub fn serialize_tags_to_json_string<S: Serializer>(
    tags: &Option<String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match tags {
        Some(s) => {
            // Try to parse as JSON array and serialize as array
            if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(s) {
                arr.serialize(serializer)
            } else {
                serializer.serialize_some(s)
            }
        }
        None => serializer.serialize_none(),
    }
}

pub fn deserialize_tags_from_input<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    let v: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    match v {
        None => Ok(None),
        Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Array(arr)) => {
            Ok(Some(serde_json::to_string(&arr).unwrap_or_default()))
        }
        Some(serde_json::Value::String(s)) => {
            // If it's already a JSON array string, keep it; otherwise wrap
            if serde_json::from_str::<Vec<serde_json::Value>>(&s).is_ok() {
                Ok(Some(s))
            } else {
                Ok(Some(s))
            }
        }
        Some(other) => Ok(Some(other.to_string())),
    }
}

// Parse a JSON text column into a serde_json::Value for API responses
pub fn parse_json_column(s: &Option<String>) -> serde_json::Value {
    match s {
        Some(s) => serde_json::from_str(s).unwrap_or(serde_json::Value::Null),
        None => serde_json::Value::Null,
    }
}

// --- Database row structs ---

#[derive(Debug, sqlx::FromRow)]
pub struct ProfileRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub avatar_color: Option<String>,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ProductRow {
    pub id: String,
    pub name: String,
    #[sqlx(rename = "type")]
    pub product_type: String,
    pub description: Option<String>,
    pub url: Option<String>,
    pub price: Option<f64>,
    pub tags: Option<String>,
    pub profile_id: Option<String>,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct CampaignRow {
    pub id: String,
    pub product_id: String,
    pub profile_id: Option<String>,
    pub name: String,
    pub status: Option<String>,
    pub goal: Option<String>,
    pub target_audience: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub notes: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct PostRow {
    pub id: String,
    pub campaign_id: String,
    pub platform: String,
    pub post_type: String,
    pub platform_post_id: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub body_preview: Option<String>,
    pub target_community: Option<String>,
    pub posted_at: Option<NaiveDateTime>,
    pub tags: Option<String>,
    pub is_api_tracked: i32,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct MetricSnapshotRow {
    pub id: i64,
    pub post_id: String,
    pub snapshot_date: String,
    pub views: Option<i64>,
    pub impressions: Option<i64>,
    pub likes: Option<i64>,
    pub dislikes: Option<i64>,
    pub comments: Option<i64>,
    pub shares: Option<i64>,
    pub saves: Option<i64>,
    pub clicks: Option<i64>,
    pub watch_time_seconds: Option<i64>,
    pub followers_gained: Option<i64>,
    pub custom_metrics: Option<String>,
    pub fetched_via: Option<String>,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct AIAnalysisRow {
    pub id: String,
    pub campaign_id: Option<String>,
    pub analysis_type: String,
    pub summary: String,
    pub top_performers: Option<String>,
    pub underperformers: Option<String>,
    pub patterns: Option<String>,
    pub recommendations: Option<String>,
    pub raw_response: Option<String>,
    pub model_used: Option<String>,
    pub tokens_used: Option<i64>,
    pub analyzed_at: Option<NaiveDateTime>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct SystemStateRow {
    pub key: String,
    pub value: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct PlatformConfigRow {
    pub platform: String,
    pub credentials: Option<String>,
    pub is_enabled: Option<i32>,
    pub rate_limit_remaining: Option<i64>,
    pub last_fetched_at: Option<NaiveDateTime>,
    pub config: Option<String>,
}
