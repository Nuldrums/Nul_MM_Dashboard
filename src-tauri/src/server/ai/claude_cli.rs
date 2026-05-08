use std::process::Command;

const RATE_LIMIT_PHRASES: &[&str] = &[
    "rate limit", "usage limit", "token limit",
    "too many requests", "quota", "capacity", "exceeded",
    "try again later", "billing",
];

#[derive(Debug)]
pub enum CliError {
    NotFound,
    RateLimited(String),
    ExecutionFailed(String),
    ParseError(String),
    Timeout,
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::NotFound => write!(f, "Claude CLI not found in PATH"),
            CliError::RateLimited(msg) => write!(f, "Claude CLI rate limited: {}", msg),
            CliError::ExecutionFailed(msg) => write!(f, "Claude CLI execution failed: {}", msg),
            CliError::ParseError(msg) => write!(f, "Claude CLI response parse error: {}", msg),
            CliError::Timeout => write!(f, "Claude CLI timed out"),
        }
    }
}

impl std::error::Error for CliError {}

/// Find the claude CLI binary in PATH.
/// On Windows, `where claude` may return a bare script shim (no .exe/.cmd extension)
/// which can't be spawned directly — causes OS error 193 "not a valid Win32 application".
/// We need to find claude.cmd or claude.exe specifically.
fn find_claude() -> Option<String> {
    // Try claude.cmd first (npm-installed CLI on Windows creates .cmd shims)
    for name in &["claude.cmd", "claude.exe", "claude"] {
        if let Ok(output) = Command::new("where").arg(name).output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout);
                let first_line = path.lines().next().unwrap_or("").trim();
                if !first_line.is_empty() {
                    tracing::debug!("Found Claude CLI at: {} (searched for {})", first_line, name);
                    return Some(first_line.to_string());
                }
            }
        }
    }

    tracing::debug!("Claude CLI not found in PATH");
    None
}

/// Check if the Claude CLI is available.
pub fn is_available() -> bool {
    let found = find_claude().is_some();
    tracing::debug!("Claude CLI available: {}", found);
    found
}

/// Call Claude via the CLI using subscription tokens.
///
/// Key workarounds from StreamClipper:
/// - `-p` flag for subscription/OAuth auth (not API key)
/// - `--output-format json` for JSON envelope response
/// - `--tools ""` to disable tool use
/// - System prompt prepended to user message (--system-prompt is ignored by CLI)
/// - ANTHROPIC_API_KEY stripped from env to force subscription billing
/// - CREATE_NO_WINDOW on Windows to hide console
/// - UTF-8 encoding for Windows compatibility
/// - Rate limit detection on exit code 0 (CLI returns 0 but outputs error text)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_error_display() {
        assert_eq!(format!("{}", CliError::NotFound), "Claude CLI not found in PATH");
        assert!(format!("{}", CliError::RateLimited("test".into())).contains("rate limited"));
        assert!(format!("{}", CliError::ExecutionFailed("oops".into())).contains("oops"));
        assert!(format!("{}", CliError::ParseError("bad json".into())).contains("bad json"));
        assert_eq!(format!("{}", CliError::Timeout), "Claude CLI timed out");
    }

    #[test]
    fn test_rate_limit_phrases() {
        let phrases_to_check = vec![
            "you've hit your rate limit",
            "usage limit exceeded",
            "too many requests please try again later",
            "token limit reached",
            "quota exceeded",
        ];
        for phrase in phrases_to_check {
            let lower = phrase.to_lowercase();
            let matched = RATE_LIMIT_PHRASES.iter().any(|p| lower.contains(p));
            assert!(matched, "Should detect rate limit in: {}", phrase);
        }
    }

    #[test]
    fn test_rate_limit_no_false_positives() {
        let normal_texts = vec![
            "the analysis is complete",
            "here are your results",
            "campaign performing well",
        ];
        for text in normal_texts {
            let lower = text.to_lowercase();
            let matched = RATE_LIMIT_PHRASES.iter().any(|p| lower.contains(p));
            assert!(!matched, "Should NOT detect rate limit in: {}", text);
        }
    }
}

pub async fn call_claude_cli(
    model: &str,
    system: &str,
    user_message: &str,
) -> Result<String, CliError> {
    let claude_path = find_claude().ok_or(CliError::NotFound)?;
    tracing::info!("Claude CLI path: {}", claude_path);
    tracing::info!("Claude CLI call: model={}, system_len={}, message_len={}",
        model, system.len(), user_message.len());

    // Prepend system prompt to user message — CLI ignores --system-prompt
    let combined_input = if system.is_empty() {
        user_message.to_string()
    } else {
        format!("{}\n\n---\n\n{}", system, user_message)
    };

    let path = claude_path.clone();
    let model = model.to_string();
    let input = combined_input.clone();

    // Run subprocess in a blocking thread to avoid blocking the async runtime
    let result = tokio::task::spawn_blocking(move || {
        // Build environment with ANTHROPIC_API_KEY stripped
        // This forces claude -p to use OAuth/subscription auth
        let env: Vec<(String, String)> = std::env::vars()
            .filter(|(k, _)| k != "ANTHROPIC_API_KEY")
            .collect();

        #[cfg(target_os = "windows")]
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let mut cmd = Command::new(&path);
        cmd.args(["-p", "--model", &model, "--output-format", "json", "--tools", ""])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .env_clear();

        // Re-add all env vars except ANTHROPIC_API_KEY
        for (k, v) in &env {
            cmd.env(k, v);
        }

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        use std::io::Write;
        tracing::info!("Spawning: {} -p --model {} --output-format json --tools \"\"", &path, &model);

        let mut child = cmd.spawn()
            .map_err(|e| CliError::ExecutionFailed(format!("Failed to spawn claude: {}", e)))?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(input.as_bytes());
            // Drop stdin to signal EOF — without this the CLI may hang waiting for input
            drop(stdin);
        }

        // Wait with timeout (5 minutes, matching StreamClipper implementation)
        let timeout = std::time::Duration::from_secs(300);
        let start = std::time::Instant::now();

        loop {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    // Process finished — collect output
                    let output = child.wait_with_output()
                        .map_err(|e| CliError::ExecutionFailed(format!("Failed to read output: {}", e)))?;
                    return Ok::<_, CliError>((output.status, output.stdout, output.stderr));
                }
                Ok(None) => {
                    // Still running — check timeout
                    if start.elapsed() > timeout {
                        let _ = child.kill();
                        tracing::error!("Claude CLI timed out after 5 minutes, killed process");
                        return Err(CliError::Timeout);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
                Err(e) => {
                    return Err(CliError::ExecutionFailed(format!("Failed to check process status: {}", e)));
                }
            }
        }
    }).await.map_err(|e| CliError::ExecutionFailed(format!("Task join error: {}", e)))??;

    let (status, stdout_bytes, stderr_bytes) = result;
    let stdout = String::from_utf8_lossy(&stdout_bytes).trim().to_string();
    let stderr = String::from_utf8_lossy(&stderr_bytes).trim().to_string();

    tracing::debug!("Claude CLI exit={}, stdout_len={}, stderr_len={}", status, stdout.len(), stderr.len());

    if !stderr.is_empty() {
        tracing::debug!("Claude CLI stderr: {}", &stderr[..stderr.len().min(500)]);
    }

    // Check for rate limits — CLI can exit 0 but output rate limit text
    let combined_output = format!("{} {}", stdout, stderr).to_lowercase();
    let is_rate_limited = RATE_LIMIT_PHRASES.iter().any(|p| combined_output.contains(p));

    if is_rate_limited {
        return Err(CliError::RateLimited(
            format!("stdout: {}... stderr: {}...",
                &stdout[..stdout.len().min(200)],
                &stderr[..stderr.len().min(200)])
        ));
    }

    if !status.success() {
        return Err(CliError::ExecutionFailed(
            format!("Exit code: {:?}, stderr: {}", status.code(), &stderr[..stderr.len().min(500)])
        ));
    }

    if stdout.is_empty() {
        return Err(CliError::ParseError("Empty stdout from Claude CLI".into()));
    }

    // Parse JSON envelope: {"result": "...", "is_error": false}
    match serde_json::from_str::<serde_json::Value>(&stdout) {
        Ok(envelope) => {
            // Check is_error field
            if envelope.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false) {
                let error_text = envelope.get("result")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error");

                // Check if the error is a rate limit
                let error_lower = error_text.to_lowercase();
                if RATE_LIMIT_PHRASES.iter().any(|p| error_lower.contains(p)) {
                    return Err(CliError::RateLimited(error_text.to_string()));
                }
                return Err(CliError::ExecutionFailed(error_text.to_string()));
            }

            // Extract the result content
            match envelope.get("result").and_then(|v| v.as_str()) {
                Some(content) => Ok(content.to_string()),
                None => {
                    // Maybe the entire response is the content (older CLI?)
                    Ok(stdout)
                }
            }
        }
        Err(_) => {
            // Fallback: stdout wasn't a JSON envelope, return raw
            // But first check for rate limit phrases
            let stdout_lower = stdout.to_lowercase();
            if RATE_LIMIT_PHRASES.iter().any(|p| stdout_lower.contains(p)) {
                return Err(CliError::RateLimited(stdout));
            }
            Ok(stdout)
        }
    }
}
