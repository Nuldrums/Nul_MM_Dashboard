use std::sync::Arc;
use serde_json::json;
use crate::server::AppState;
use crate::server::ai::prompts;

const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

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

async fn call_claude(
    state: &Arc<AppState>,
    system: &str,
    user_message: &str,
) -> anyhow::Result<serde_json::Value> {
    let api_key = &state.settings.anthropic_api_key;
    if api_key.is_empty() {
        anyhow::bail!("Anthropic API key not configured");
    }

    tracing::info!("Calling Claude API (model={})", DEFAULT_MODEL);

    let body = json!({
        "model": DEFAULT_MODEL,
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
        "model_used": DEFAULT_MODEL,
        "tokens_used": input_tokens + output_tokens,
    }))
}

fn parse_json_response(raw: &str) -> anyhow::Result<serde_json::Value> {
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
