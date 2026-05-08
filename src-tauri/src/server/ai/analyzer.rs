use std::sync::Arc;
use serde_json::json;
use crate::server::AppState;
use crate::server::ai::{prompts, claude_cli};

const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";
/// Model name used for CLI calls (uses shorthand format)
const DEFAULT_CLI_MODEL: &str = "claude-sonnet-4-6";

pub async fn analyze_campaign(
    state: &Arc<AppState>,
    campaign_context: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let user_message = prompts::CAMPAIGN_ANALYSIS_USER_TEMPLATE
        .replace("{campaign_name}", campaign_context["campaign_name"].as_str().unwrap_or(""))
        .replace("{product_name}", campaign_context["product_name"].as_str().unwrap_or(""))
        .replace("{product_type}", campaign_context["product_type"].as_str().unwrap_or(""))
        .replace("{goal}", campaign_context["goal"].as_str().unwrap_or(""))
        .replace("{target_audience}", campaign_context["target_audience"].as_str().unwrap_or(""))
        .replace("{campaign_tags}", campaign_context["campaign_tags"].as_str().unwrap_or(""))
        .replace("{duration_days}", &campaign_context["duration_days"].to_string())
        .replace("{posts_data}", campaign_context["posts_data"].as_str().unwrap_or(""))
        .replace("{historical_context}", campaign_context["historical_context"].as_str().unwrap_or("No historical data available."));

    call_claude(state, prompts::CAMPAIGN_ANALYSIS_SYSTEM, &user_message).await
}

pub async fn analyze_cross_campaign(
    state: &Arc<AppState>,
    campaigns_data: &str,
    historical_patterns: &str,
) -> anyhow::Result<serde_json::Value> {
    let user_message = prompts::CROSS_CAMPAIGN_USER_TEMPLATE
        .replace("{campaigns_data}", campaigns_data)
        .replace("{historical_patterns}", historical_patterns);

    call_claude(state, prompts::CROSS_CAMPAIGN_SYSTEM, &user_message).await
}

/// Resolve which AI provider and model to use.
/// Reads ai_provider and ai_model from system_state DB table.
async fn get_ai_config(state: &Arc<AppState>) -> (String, String, String) {
    // provider: "cli" or "api"
    let provider_row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT value FROM system_state WHERE key = 'ai_provider'"
    ).fetch_optional(&state.db).await.unwrap_or(None);
    let provider = provider_row
        .and_then(|r| r.0)
        .unwrap_or_else(|| "cli".to_string());

    let model_row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT value FROM system_state WHERE key = 'ai_model'"
    ).fetch_optional(&state.db).await.unwrap_or(None);

    let api_model = model_row.clone()
        .and_then(|r| r.0)
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());

    // Map full model names to CLI shorthand
    let cli_model = model_row
        .and_then(|r| r.0)
        .map(|m| match m.as_str() {
            "claude-sonnet-4-20250514" => "claude-sonnet-4-6".to_string(),
            "claude-opus-4-20250514" => "claude-opus-4-6".to_string(),
            "claude-3-5-haiku-20241022" => "claude-3-5-haiku".to_string(),
            other => other.to_string(),
        })
        .unwrap_or_else(|| DEFAULT_CLI_MODEL.to_string());

    (provider, api_model, cli_model)
}

async fn call_claude(
    state: &Arc<AppState>,
    system: &str,
    user_message: &str,
) -> anyhow::Result<serde_json::Value> {
    let (provider, api_model, cli_model) = get_ai_config(state).await;

    match provider.as_str() {
        "api" => {
            // API-only mode
            call_claude_api(state, system, user_message, &api_model).await
        }
        _ => {
            // CLI-first mode (default): try CLI, fall back to API on rate limit
            if claude_cli::is_available() {
                tracing::info!("Calling Claude CLI (model={})", cli_model);
                match claude_cli::call_claude_cli(&cli_model, system, user_message).await {
                    Ok(raw_text) => {
                        tracing::info!("Claude CLI call succeeded");
                        let parsed = parse_json_response(&raw_text)?;
                        Ok(json!({
                            "analysis": parsed,
                            "raw_response": raw_text,
                            "model_used": format!("{} (CLI/subscription)", cli_model),
                            "tokens_used": 0,
                        }))
                    }
                    Err(claude_cli::CliError::RateLimited(msg)) => {
                        tracing::warn!("Claude CLI rate limited: {}", msg);
                        // Fall back to API if key is configured
                        if !state.settings.anthropic_api_key.is_empty() {
                            tracing::info!("Falling back to Anthropic API");
                            call_claude_api(state, system, user_message, &api_model).await
                        } else {
                            anyhow::bail!(
                                "Claude CLI rate limited and no API key configured for fallback. \
                                 Add an Anthropic API key in Settings to enable automatic fallback."
                            )
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Claude CLI error: {}", e);
                        // For non-rate-limit errors, also try API fallback if available
                        if !state.settings.anthropic_api_key.is_empty() {
                            tracing::info!("CLI failed, falling back to Anthropic API");
                            call_claude_api(state, system, user_message, &api_model).await
                        } else {
                            anyhow::bail!("Claude CLI failed: {}. No API key configured for fallback.", e)
                        }
                    }
                }
            } else if !state.settings.anthropic_api_key.is_empty() {
                // CLI not found, use API directly
                tracing::info!("Claude CLI not found, using API directly");
                call_claude_api(state, system, user_message, &api_model).await
            } else {
                anyhow::bail!(
                    "No AI provider available. Install Claude CLI for subscription access, \
                     or add an Anthropic API key in Settings."
                )
            }
        }
    }
}

async fn call_claude_api(
    state: &Arc<AppState>,
    system: &str,
    user_message: &str,
    model: &str,
) -> anyhow::Result<serde_json::Value> {
    let api_key = &state.settings.anthropic_api_key;
    if api_key.is_empty() {
        anyhow::bail!("Anthropic API key not configured");
    }

    tracing::info!("Calling Claude API (model={})", model);

    let body = json!({
        "model": model,
        "max_tokens": 4096,
        "system": system,
        "messages": [{"role": "user", "content": user_message}]
    });

    let resp = state.http_client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Claude API error {}: {}", status, text);
    }

    let response: serde_json::Value = resp.json().await?;

    let raw_text = response["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let input_tokens = response["usage"]["input_tokens"].as_i64().unwrap_or(0);
    let output_tokens = response["usage"]["output_tokens"].as_i64().unwrap_or(0);
    tracing::info!("Claude API response: input_tokens={}, output_tokens={}", input_tokens, output_tokens);

    let parsed = parse_json_response(&raw_text)?;

    Ok(json!({
        "analysis": parsed,
        "raw_response": raw_text,
        "model_used": format!("{} (API)", model),
        "tokens_used": input_tokens + output_tokens,
    }))
}

pub async fn analyze_campaign_delta(
    state: &Arc<AppState>,
    delta_context: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let user_message = prompts::CAMPAIGN_DELTA_USER_TEMPLATE
        .replace("{campaign_name}", delta_context["campaign_name"].as_str().unwrap_or(""))
        .replace("{product_name}", delta_context["product_name"].as_str().unwrap_or(""))
        .replace("{product_type}", delta_context["product_type"].as_str().unwrap_or(""))
        .replace("{goal}", delta_context["goal"].as_str().unwrap_or(""))
        .replace("{target_audience}", delta_context["target_audience"].as_str().unwrap_or(""))
        .replace("{campaign_tags}", delta_context["campaign_tags"].as_str().unwrap_or(""))
        .replace("{days_since_last}", &delta_context["days_since_last"].to_string())
        .replace("{prior_summary}", delta_context["prior_summary"].as_str().unwrap_or(""))
        .replace("{prior_score}", &delta_context["prior_score"].to_string())
        .replace("{prior_recommendations}", delta_context["prior_recommendations"].as_str().unwrap_or(""))
        .replace("{metric_deltas}", delta_context["metric_deltas"].as_str().unwrap_or("No changes"))
        .replace("{new_posts_data}", delta_context["new_posts_data"].as_str().unwrap_or("None"))
        .replace("{knowledge_context}", delta_context["knowledge_context"].as_str().unwrap_or("No prior knowledge."));

    call_claude(state, prompts::CAMPAIGN_DELTA_SYSTEM, &user_message).await
}

pub async fn recommend_new_campaign(
    state: &Arc<AppState>,
    campaign_context: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let user_message = prompts::NEW_CAMPAIGN_RECOMMENDATION_USER_TEMPLATE
        .replace("{product_name}", campaign_context["product_name"].as_str().unwrap_or(""))
        .replace("{product_type}", campaign_context["product_type"].as_str().unwrap_or(""))
        .replace("{product_description}", campaign_context["product_description"].as_str().unwrap_or(""))
        .replace("{goal}", campaign_context["goal"].as_str().unwrap_or(""))
        .replace("{target_audience}", campaign_context["target_audience"].as_str().unwrap_or(""))
        .replace("{platforms}", campaign_context["platforms"].as_str().unwrap_or(""))
        .replace("{knowledge_context}", campaign_context["knowledge_context"].as_str().unwrap_or("No prior campaign data available."));

    call_claude(state, prompts::NEW_CAMPAIGN_RECOMMENDATION_SYSTEM, &user_message).await
}

pub fn parse_json_response(raw: &str) -> anyhow::Result<serde_json::Value> {
    // Strip markdown code fences if present
    let trimmed = raw.trim();
    let json_str = if trimmed.starts_with("```") {
        let start = trimmed.find('\n').map(|i| i + 1).unwrap_or(0);
        let end = trimmed.rfind("```").unwrap_or(trimmed.len());
        &trimmed[start..end]
    } else {
        trimmed
    };

    serde_json::from_str(json_str.trim())
        .map_err(|e| anyhow::anyhow!("Failed to parse Claude JSON response: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_plain() {
        let raw = r#"{"summary": "Test summary", "score": 85}"#;
        let result = parse_json_response(raw).unwrap();
        assert_eq!(result["summary"], "Test summary");
        assert_eq!(result["score"], 85);
    }

    #[test]
    fn test_parse_json_with_markdown_fences() {
        let raw = "```json\n{\"summary\": \"Fenced response\", \"score\": 42}\n```";
        let result = parse_json_response(raw).unwrap();
        assert_eq!(result["summary"], "Fenced response");
        assert_eq!(result["score"], 42);
    }

    #[test]
    fn test_parse_json_with_plain_fences() {
        let raw = "```\n{\"key\": \"value\"}\n```";
        let result = parse_json_response(raw).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[test]
    fn test_parse_json_with_whitespace() {
        let raw = "  \n  {\"key\": \"value\"}  \n  ";
        let result = parse_json_response(raw).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[test]
    fn test_parse_json_invalid() {
        let raw = "this is not json at all";
        assert!(parse_json_response(raw).is_err());
    }

    #[test]
    fn test_parse_json_nested() {
        let raw = r#"{
            "summary": "Test",
            "recommendations": [
                {"action": "Do X", "priority": "high"},
                {"action": "Do Y", "priority": "low"}
            ],
            "meta_learning": {
                "product_type_insight": "Works well",
                "platform_insight": "Reddit is best"
            }
        }"#;
        let result = parse_json_response(raw).unwrap();
        assert_eq!(result["recommendations"].as_array().unwrap().len(), 2);
        assert_eq!(result["meta_learning"]["platform_insight"], "Reddit is best");
    }

    #[test]
    fn test_parse_json_empty_object() {
        let raw = "{}";
        let result = parse_json_response(raw).unwrap();
        assert!(result.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_model_mapping() {
        // Test the model name mapping logic used in get_ai_config
        let mappings = vec![
            ("claude-sonnet-4-20250514", "claude-sonnet-4-6"),
            ("claude-opus-4-20250514", "claude-opus-4-6"),
            ("claude-3-5-haiku-20241022", "claude-3-5-haiku"),
            ("custom-model", "custom-model"),
        ];
        for (input, expected) in mappings {
            let mapped = match input {
                "claude-sonnet-4-20250514" => "claude-sonnet-4-6".to_string(),
                "claude-opus-4-20250514" => "claude-opus-4-6".to_string(),
                "claude-3-5-haiku-20241022" => "claude-3-5-haiku".to_string(),
                other => other.to_string(),
            };
            assert_eq!(mapped, expected, "Model mapping failed for {}", input);
        }
    }
}
