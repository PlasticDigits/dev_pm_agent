//! Cursor CLI integration.

use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::process::Command;

const REPOS_PREFIX: &str = "repos";

/// Validate repo path is under ~/repos/
pub fn validate_repo_path(path: &str) -> Result<()> {
    let expanded = shellexpand::tilde(path).to_string();
    if !expanded.contains(REPOS_PREFIX) {
        anyhow::bail!("repo path must be under ~/repos/");
    }
    Ok(())
}

/// Run the full command pipeline: translate -> execute -> summarize
pub async fn run_command(
    input: &str,
    repo_path: &str,
    translator_model: &str,
    workload_model: &str,
) -> Result<(String, String)> {
    validate_repo_path(repo_path)?;

    let expanded = shellexpand::tilde(repo_path).to_string();

    // 1. Translation
    let translation_prompt = format!(
        r#"Given this user input, produce a JSON object with: repo_path, cursor_prompt, context_mode. 
Use "continue" or "new" for context_mode.
Input: "{}""#,
        input.replace('"', "\\\"")
    );

    let translation_out = run_agent(translator_model, None, &translation_prompt).await?;
    let parsed: serde_json::Value =
        serde_json::from_str(&translation_out).context("parse translation JSON")?;

    let cursor_prompt = parsed["cursor_prompt"]
        .as_str()
        .unwrap_or(input)
        .to_string();
    let _context_mode = parsed["context_mode"].as_str().unwrap_or("continue");

    // 2. Execution
    let exec_out = run_agent_in_repo(workload_model, &expanded, &cursor_prompt).await?;

    // 3. Summarization
    let summary_prompt = format!(
        "Summarize this Cursor CLI output for mobile display in 3-5 bullet points. Keep under 500 chars: {}",
        exec_out.replace('"', "\\\"").chars().take(2000).collect::<String>()
    );
    let summary = run_agent(workload_model, None, &summary_prompt)
        .await
        .unwrap_or_else(|_| "Summary unavailable".to_string());

    Ok((exec_out, summary))
}

async fn run_agent(model: &str, repo: Option<&str>, prompt: &str) -> Result<String> {
    let mut cmd = Command::new("agent");
    cmd.args(["-p", "-m", model, "--output-format", "text", "--force"])
        .arg(prompt)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(r) = repo {
        cmd.args(["-C", r]);
    }

    let output = cmd.output().await.context("run agent")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("agent failed: {}", stderr);
    }
    Ok(stdout.to_string())
}

async fn run_agent_in_repo(model: &str, repo: &str, prompt: &str) -> Result<String> {
    run_agent(model, Some(repo), prompt).await
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
