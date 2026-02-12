//! Cursor CLI integration.

use anyhow::{Context, Result};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;

const REPOS_PREFIX: &str = "repos";

/// Extract JSON object from text that may be wrapped in markdown or extra prose.
fn extract_json(s: &str) -> Option<&str> {
    let s = s.trim();
    if let Some(start) = s.find('{') {
        if let Some(end) = s.rfind('}') {
            if end >= start {
                return Some(s.get(start..=end)?);
            }
        }
    }
    None
}

/// Validate repo path is under ~/repos/
pub fn validate_repo_path(path: &str) -> Result<()> {
    let expanded = shellexpand::tilde(path).to_string();
    if !expanded.contains(REPOS_PREFIX) {
        anyhow::bail!("repo path must be under ~/repos/");
    }
    Ok(())
}

/// Callback for streaming output. Receives current accumulated output.
pub type OnOutput = Arc<dyn Fn(&str) + Send + Sync>;

/// Create a new Cursor CLI chat and return its ID for use with --resume.
async fn create_cursor_chat() -> Result<String> {
    let output = Command::new("agent")
        .args(["create-chat"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("run agent create-chat")?;
    let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if id.is_empty() {
        anyhow::bail!("agent create-chat returned empty");
    }
    Ok(id)
}

const FOLDER_PLACEMENT: &str = r#"
Template output folder placement: Put documents in plans/, security_reviews/, or sprints/ as appropriate. Use repo root (e.g. sprints/SPRINT_001.md) for repo-wide scope; if the work only touches one package in a monorepo, use that package's subdirectory (e.g. packages/foo/sprints/SPRINT_001.md).
"#;

/// Build template-specific context instructions for the translation prompt.
/// When context_mode is set, these guide the model on the expected workflow.
fn context_mode_instructions(context_mode: Option<&str>, repo_path: &str) -> String {
    let escaped_repo = repo_path.replace('"', "\\\"");
    match context_mode {
        Some("sprint") => format!(
            r#"Context: User selected SPRINT template. You are in workspace: "{}".
{}
Sprint workflow: Create a sprint document in sprints/ (or packages/{{name}}/sprints/ if only touching one package). Use SPRINT_001.md or next available number. Base it on the user's request and any prior work (security audit, gap analysis, etc.).
Write the sprint doc to the repo. Output its full contents for user review.
IMPORTANT: Do NOT implement the changes yet. The user will review the sprint doc, then send a follow-up message (e.g. "implement it" or "approved") to execute the sprint.
Exception: If the user explicitly asks to "implement" or "execute" an existing sprint, do that instead of creating a new doc.
"#,
            escaped_repo, FOLDER_PLACEMENT
        ),
        Some("security_review") => format!(
            r#"Context: User selected SECURITY REVIEW template. Workspace: "{}".
{}
Produce a security review document. Place it in security_reviews/ at root, or packages/{{name}}/security_reviews/ if only touching one package. Include findings summary, severity levels, and recommended actions.
"#,
            escaped_repo, FOLDER_PLACEMENT
        ),
        Some("monorepo_init") => format!(
            r#"Context: User selected MONOREPO INIT template. Workspace: "{}".
{}
Create plans/ folder and PLAN_INITIAL.md (at root or packages/{{name}}/plans/ if package-scoped). Output for interactive review.
"#,
            escaped_repo, FOLDER_PLACEMENT
        ),
        Some("gap_analysis") => format!(
            r#"Context: User selected GAP ANALYSIS template. Workspace: "{}".
{}
Write PLAN_GAP_{{x}}.md in plans/ (or packages/{{name}}/plans/ if package-scoped). Output for interactive review.
"#,
            escaped_repo, FOLDER_PLACEMENT
        ),
        Some("feature_plan") => format!(
            r#"Context: User selected FEATURE PLAN template. Workspace: "{}".
{}
Write PLAN_FEAT_{{x}}.md in plans/ (or packages/{{name}}/plans/ if package-scoped). Output for interactive review.
"#,
            escaped_repo, FOLDER_PLACEMENT
        ),
        Some("commit") => format!(
            r#"Context: User selected COMMIT template. Workspace: "{}".
Commit the changes made in this conversation (stage and commit with an appropriate message). Pre-commit hooks (lint, test, format) often run and can take a long time. In your output, clearly describe what happened: whether pre-commit passed or failed, what ran, and any errors if it failed. The user needs to know the outcome either way. NEVER use --no-verify.
Output format for pre-commit report: Keep it compact. Put section numbers and headers on the same line (e.g. "1. Format checks" not "1.\nFormat checks"). Use a proper Markdown unordered list (-) for each check item so they render as separate list items (e.g. "- operator (Rust fmt) – ✓" on its own line), not as continuation of the section header.
"#,
            escaped_repo
        ),
        _ => String::new(),
    }
}

/// Format chat history for the translation prompt.
fn format_chat_history(history: &[(String, Option<String>)]) -> String {
    let mut out = String::new();
    for (input, output) in history {
        out.push_str("User: ");
        out.push_str(input);
        out.push('\n');
        if let Some(ref o) = output {
            out.push_str("Assistant: ");
            out.push_str(o);
            out.push('\n');
        }
    }
    out
}

/// Run the full command pipeline: translate -> execute -> summarize.
/// Uses `agent create-chat` to get a session ID (or `resume_chat_id` when continuing),
/// then runs workload with `--resume [chatId]`.
/// Returns (output, summary, cursor_chat_id).
pub async fn run_command(
    input: &str,
    repo_path: &str,
    translator_model: &str,
    workload_model: &str,
    resume_chat_id: Option<&str>,
    on_output: Option<OnOutput>,
    context_mode: Option<&str>,
    chat_history: Option<&[(String, Option<String>)]>,
) -> Result<(String, String, String)> {
    validate_repo_path(repo_path)?;

    let expanded = shellexpand::tilde(repo_path).to_string();

    // 1. Translation (skip for freeform — send user input directly)
    let cursor_prompt = if context_mode.is_none() {
        if let Some(ref cb) = on_output {
            cb("[Creating chat session...]");
        }
        input.to_string()
    } else {
        if let Some(ref cb) = on_output {
            cb("Translating task...");
        }
        let context_prefix = context_mode_instructions(context_mode, repo_path);
        let (history_block, input_label) = match chat_history.filter(|h| !h.is_empty()) {
            Some(h) => {
                let formatted = format_chat_history(h);
                (
                    format!("\n\nPrior conversation (for context):\n{}\n", formatted),
                    "Current user input",
                )
            }
            None => (String::new(), "Input"),
        };
        let translation_prompt = format!(
            r#"{}{}
Given this user input{}, produce a JSON object with only cursor_prompt. 
Output format: {{"cursor_prompt": "refined or expanded task for the coding agent"}}
{}{}: "{}""#,
            context_prefix,
            if context_prefix.is_empty() { "" } else { "\n" },
            if history_block.is_empty() {
                ""
            } else {
                " (and prior conversation)"
            },
            history_block,
            input_label,
            input.replace('"', "\\\"")
        );

        let translation_out = run_agent(translator_model, None, &translation_prompt).await?;
        let json_str = extract_json(&translation_out).context("no JSON in translator output")?;
        let parsed: serde_json::Value =
            serde_json::from_str(json_str).context("parse translation JSON")?;
        parsed["cursor_prompt"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("cursor_prompt missing in translator output"))?
            .to_string()
    };

    let chat_id = match resume_chat_id {
        Some(id) => {
            tracing::info!(chat_id = %id, "resuming existing Cursor chat");
            id.to_string()
        }
        None => {
            if let Some(ref cb) = on_output {
                cb(&format!(
                    "T: {}\n\n[Creating chat session...]",
                    cursor_prompt
                ));
            }
            create_cursor_chat().await?
        }
    };

    if let Some(ref cb) = on_output {
        cb(&format!("T: {}\n\n[Running agent...]", cursor_prompt));
    }

    // 2. Execution (streaming) with --resume for Cursor CLI chat session
    let exec_out = run_agent_in_repo_streaming(
        workload_model,
        &expanded,
        &cursor_prompt,
        Some(&chat_id),
        on_output.as_ref().map(|cb| {
            let trans_display = format!("T: {}", cursor_prompt);
            let cb = Arc::clone(cb);
            Arc::new(move |out: &str| {
                cb(&format!("{}\n\n{}", trans_display, out));
            }) as OnOutput
        }),
    )
    .await?;

    // 3. Summarization
    if let Some(ref cb) = on_output {
        cb(&format!("{}\n\n[Summarizing...]", exec_out));
    }
    let summary_prompt = format!(
        "Summarize this Cursor CLI output as clean Markdown.\n\
Rules:\n\
- Start with a short title naming the work subject (for example: \"## Auth validation fix\").\n\
- Do not use generic titles like \"mobile-friendly summary\" or \"summary\".\n\
- Use 3-5 concise bullet points with '-' markers (never numbered lists).\n\
- Do not wrap output in quotes or code fences.\n\
- Keep the total output under 700 characters.\n\
- Return Markdown only.\n\
\n\
Output to summarize:\n{}",
        exec_out
            .replace('"', "\\\"")
            .chars()
            .take(2000)
            .collect::<String>()
    );
    let summary = run_agent(workload_model, None, &summary_prompt)
        .await
        .unwrap_or_else(|_| "Summary unavailable".to_string());

    Ok((exec_out, summary, chat_id))
}

async fn run_agent(model: &str, repo: Option<&str>, prompt: &str) -> Result<String> {
    let mut cmd = Command::new("agent");
    cmd.args(["-p", "--model", model, "--output-format", "text", "--force"])
        .arg(prompt)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(r) = repo {
        cmd.args(["--workspace", r]);
    }

    let output = cmd.output().await.context("run agent")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("agent failed: {}", stderr);
    }
    Ok(stdout.to_string())
}

/// Format a tool_call event into human-readable console output.
/// The `tool_call` object has a single key like `bashToolCall`, `lsToolCall`, etc.
/// On "started", show what tool is running. On "completed", append the result.
fn format_tool_call(
    tc: &serde_json::Map<String, serde_json::Value>,
    subtype: &str,
    console: &mut String,
) {
    // The tool name is the first (and only) key, e.g. "bashToolCall", "lsToolCall"
    let Some((tool_key, inner)) = tc.iter().next() else {
        return;
    };
    let tool_name = tool_key
        .trim_end_matches("ToolCall")
        .trim_end_matches("Tool");

    if subtype == "started" {
        // Extract a human-readable description of what the tool is doing
        let args = inner.get("args");
        let desc = match tool_name {
            "bash" | "runCommand" | "terminal" => args
                .and_then(|a| a.get("command").or_else(|| a.get("cmd")))
                .and_then(|v| v.as_str())
                .map(|s| format!("$ {}", s)),
            "ls" | "listDir" => args
                .and_then(|a| a.get("path"))
                .and_then(|v| v.as_str())
                .map(|s| format!("ls {}", s)),
            "read" | "readFile" => args
                .and_then(|a| a.get("path").or_else(|| a.get("filePath")))
                .and_then(|v| v.as_str())
                .map(|s| format!("cat {}", s)),
            "write" | "writeFile" | "editFile" | "edit" => args
                .and_then(|a| a.get("path").or_else(|| a.get("filePath")))
                .and_then(|v| v.as_str())
                .map(|s| format!("write {}", s)),
            "grep" | "search" => args
                .and_then(|a| a.get("pattern").or_else(|| a.get("query")))
                .and_then(|v| v.as_str())
                .map(|s| format!("grep {}", s)),
            _ => Some(format!("[{}]", tool_name)),
        };
        if let Some(d) = desc {
            if !console.is_empty() {
                console.push('\n');
            }
            console.push_str(&d);
            console.push_str(" ...");
        }
    } else if subtype == "completed" {
        // Extract result summary — look for common result patterns
        let result = inner.get("result");
        if let Some(r) = result {
            // Check for error
            if let Some(err) = r.get("error").and_then(|v| v.as_str()) {
                console.push_str(&format!("\n✗ {}", err));
                return;
            }
            // For bash: result might have stdout/output
            if let Some(output) = r
                .get("stdout")
                .or_else(|| r.get("output"))
                .and_then(|v| v.as_str())
            {
                let trimmed = output.trim();
                if !trimmed.is_empty() {
                    // Cap output at 2000 chars for streaming
                    let preview: String = trimmed.chars().take(2000).collect();
                    console.push_str(&format!("\n{}", preview));
                    if trimmed.len() > 2000 {
                        console.push_str("\n... (truncated)");
                    }
                }
                return;
            }
            // For success objects, just note completion
            if r.get("success").is_some() {
                console.push_str(" ✓");
            }
        }
    }
}

async fn run_agent_in_repo_streaming(
    model: &str,
    repo: &str,
    prompt: &str,
    chat_id: Option<&str>,
    on_output: Option<OnOutput>,
) -> Result<String> {
    tracing::info!(
        model = model,
        repo = repo,
        prompt_len = prompt.len(),
        chat_id = chat_id,
        "spawning agent"
    );

    let mut cmd = Command::new("agent");
    cmd.args([
        "-p",
        "--model",
        model,
        "--output-format",
        "stream-json",
        "--stream-partial-output",
        "--force",
    ]);
    if let Some(id) = chat_id {
        cmd.args(["--resume", id]);
    }
    cmd.arg(prompt)
        .args(["--workspace", repo])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().context("spawn agent")?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("no stdout"))?;
    let stderr = child.stderr.take();

    let (tx, rx) = tokio::sync::oneshot::channel::<Result<String>>();
    let mut reader = tokio::io::BufReader::new(stdout);
    let mut buf = String::new();
    let stream_cb = on_output.clone();

    if let Some(stderr) = stderr {
        tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stderr);
            let mut line = String::new();
            while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    tracing::warn!(stderr = %trimmed, "agent stderr");
                }
                line.clear();
            }
        });
    }

    tokio::spawn(async move {
        let mut thinking = String::new();
        let mut response = String::new();
        let mut full_result = String::new();
        let mut console = String::new();
        let mut event_count = 0u32;
        loop {
            buf.clear();
            match reader.read_line(&mut buf).await {
                Ok(0) => break,
                Ok(_) => {
                    let line = buf.trim();
                    if line.is_empty() {
                        continue;
                    }
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                        event_count += 1;
                        let ty = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        let subtype = val.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
                        tracing::debug!(
                            event = event_count,
                            ty = ty,
                            subtype = subtype,
                            "stream event"
                        );
                        match (ty, subtype) {
                            ("thinking", "delta") => {
                                if let Some(t) = val.get("text").and_then(|v| v.as_str()) {
                                    thinking.push_str(t);
                                }
                            }
                            ("assistant", "delta") => {
                                // Delta events: small chunks of new text; append.
                                if let Some(t) = val
                                    .get("message")
                                    .and_then(|m| m.get("content"))
                                    .and_then(|c| c.get(0))
                                    .and_then(|x| x.get("text"))
                                    .and_then(|v| v.as_str())
                                {
                                    response.push_str(t);
                                }
                            }
                            ("assistant", _) => {
                                // Full/accumulated assistant message: replace to avoid
                                // duplication when both delta and full events are emitted.
                                if let Some(t) = val
                                    .get("message")
                                    .and_then(|m| m.get("content"))
                                    .and_then(|c| c.get(0))
                                    .and_then(|x| x.get("text"))
                                    .and_then(|v| v.as_str())
                                {
                                    response = t.to_string();
                                }
                            }
                            ("tool_call", sub) => {
                                if let Some(tc) = val.get("tool_call").and_then(|v| v.as_object()) {
                                    format_tool_call(tc, sub, &mut console);
                                }
                            }
                            ("result", _) => {
                                if let Some(r) = val.get("result").and_then(|v| v.as_str()) {
                                    full_result = r.to_string();
                                    tracing::info!(
                                        result_len = full_result.len(),
                                        "agent result received"
                                    );
                                }
                            }
                            _ => {
                                tracing::debug!(
                                    event = event_count,
                                    ty = ty,
                                    subtype = subtype,
                                    "unhandled stream event"
                                );
                            }
                        }
                        let mut display = String::new();
                        if !thinking.is_empty() {
                            display.push_str("[Thinking]\n");
                            display.push_str(&thinking);
                            display.push_str("\n\n");
                        }
                        if !console.is_empty() {
                            display.push_str("[Console]\n");
                            display.push_str(&console);
                            display.push_str("\n\n");
                        }
                        let response_content = if full_result.is_empty() {
                            &response
                        } else {
                            &full_result
                        };
                        if !response_content.trim().is_empty() {
                            display.push_str("[Response]\n");
                            display.push_str(response_content);
                        }
                        if !display.is_empty() && stream_cb.is_some() {
                            stream_cb.as_ref().unwrap()(&display);
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e.into()));
                    return;
                }
            }
        }
        let result = if full_result.is_empty() {
            if !thinking.is_empty() {
                format!("[Thinking]\n{}\n\n[Response]\n{}", thinking, response)
            } else {
                response
            }
        } else {
            full_result
        };
        tracing::info!(events = event_count, "agent stream complete");
        let _ = tx.send(Ok(result));
    });

    tracing::info!("waiting for agent process to exit");
    let status = child.wait().await.context("wait agent")?;
    if !status.success() {
        let mut stderr = String::new();
        if let Some(mut s) = child.stderr {
            let _ = tokio::io::AsyncReadExt::read_to_string(&mut s, &mut stderr).await;
        }
        anyhow::bail!("agent failed: {}", stderr);
    }

    rx.await.map_err(|_| anyhow::anyhow!("read task dropped"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_repo_path_accepts_tilde_repos() {
        assert!(validate_repo_path("~/repos/foo").is_ok());
        assert!(validate_repo_path("~/repos/foo/bar").is_ok());
    }

    #[test]
    fn validate_repo_path_accepts_repos_substring() {
        // Paths that expand to something containing "repos"
        assert!(validate_repo_path("~/repos/xyz").is_ok());
        assert!(validate_repo_path("/home/user/repos/project").is_ok());
    }

    #[test]
    fn validate_repo_path_rejects_non_repos() {
        assert!(validate_repo_path("/tmp/foo").is_err());
        assert!(validate_repo_path("~/documents").is_err());
    }
}
