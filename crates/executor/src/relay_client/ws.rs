//! WebSocket client for receiving commands from relayer.

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use shared::{
    FileReadResponseRequest, FileSearchMatch, FileSearchResponseRequest, WsCommandNewPayload,
    WsEnvelope, WsFileReadRequestPayload, WsFileSearchRequestPayload,
};
use std::io;
use std::path::{Path, PathBuf};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::cursor;

/// Normalize file_path: strip leading slash and ./ so repo.join() works correctly.
fn normalize_file_path(file_path: &str) -> &str {
    file_path
        .trim()
        .trim_start_matches('/')
        .trim_start_matches("./")
}

/// Read a file from the repo and POST content/error to relayer.
async fn handle_file_read(
    base_url: &str,
    api_key: &str,
    repo_path: &str,
    file_path: &str,
    request_id: Uuid,
) -> Result<()> {
    cursor::validate_repo_path(repo_path)?;
    let normalized_path = normalize_file_path(file_path);
    if normalized_path.is_empty() {
        anyhow::bail!("invalid file path");
    }
    let expanded_repo = shellexpand::tilde(repo_path).to_string();
    let repo = Path::new(&expanded_repo);
    let full = repo.join(normalized_path);
    let canonical = full.canonicalize().map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            anyhow::anyhow!(
                "File not found: {} (in repo {})",
                normalized_path,
                repo_path
            )
        } else {
            anyhow::anyhow!("{}", e)
        }
    })?;
    if !canonical.starts_with(repo.canonicalize().map_err(|e| anyhow::anyhow!("{}", e))?) {
        anyhow::bail!("path traversal not allowed");
    }
    let content = tokio::fs::read_to_string(&canonical).await?;
    post_file_read_response(base_url, api_key, request_id, Some(content), None).await
}

/// Returns true if file basename matches the pattern. Supports "*.md" glob (suffix match).
fn file_matches_pattern(basename: &str, pattern: &str) -> bool {
    if pattern == "*.md" {
        basename.ends_with(".md") && basename != ".md"
    } else {
        basename == pattern
    }
}

/// Search repo for files matching file_name (exact match on basename, or "*.md" for all markdown).
/// Return matches sorted by modified time (newest first). Paths are relative to repo root.
async fn handle_file_search(
    base_url: &str,
    api_key: &str,
    repo_path: &str,
    file_name: &str,
    request_id: Uuid,
) -> Result<()> {
    cursor::validate_repo_path(repo_path)?;
    let name = file_name.trim();
    if name.is_empty() {
        post_file_search_response(
            base_url,
            api_key,
            request_id,
            None,
            Some("invalid file name".to_string()),
        )
        .await?;
        return Ok(());
    }
    let expanded_repo = shellexpand::tilde(repo_path).to_string();
    let repo = PathBuf::from(&expanded_repo);
    let repo_canon = repo
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("repo not found: {}", e))?;

    const SKIP_DIRS: &[&str] = &[
        "node_modules",
        "target",
        ".git",
        "dist",
        "build",
        "out",
        ".next",
        "coverage",
        "__pycache__",
        "venv",
        ".venv",
        "vendor",
        ".turbo",
    ];
    let mut matches: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
    for entry in WalkDir::new(&repo)
        .follow_links(false)
        .max_depth(20)
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            if n.starts_with('.') {
                return false;
            }
            !SKIP_DIRS.contains(&n.as_ref())
        })
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let basename = entry.file_name().to_string_lossy();
        if !file_matches_pattern(&basename, name) {
            continue;
        }
        let meta = match std::fs::metadata(entry.path()) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if let Ok(mtime) = meta.modified() {
            let rel = entry
                .path()
                .strip_prefix(&repo_canon)
                .map(PathBuf::from)
                .unwrap_or_else(|_| entry.path().to_path_buf());
            matches.push((rel, mtime));
        }
    }

    matches.sort_by(|a, b| b.1.cmp(&a.1)); // newest first

    let limit = if name == "*.md" { 200 } else { 50 };
    let file_matches: Vec<FileSearchMatch> = matches
        .into_iter()
        .take(limit)
        .map(|(p, mtime)| {
            let path_str = p.to_string_lossy().replace('\\', "/");
            let modified_at = mtime
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| {
                    chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_default()
                })
                .unwrap_or_else(|_| String::new());
            FileSearchMatch {
                path: path_str,
                modified_at,
            }
        })
        .collect();

    post_file_search_response(base_url, api_key, request_id, Some(file_matches), None).await
}

/// POST file search response to relayer.
async fn post_file_search_response(
    base_url: &str,
    api_key: &str,
    request_id: Uuid,
    matches: Option<Vec<FileSearchMatch>>,
    error: Option<String>,
) -> Result<()> {
    let http_url = base_url
        .replace("wss://", "https://")
        .replace("ws://", "http://");
    let url = format!(
        "{}/api/files/search/response",
        http_url.trim_end_matches("/ws")
    );
    let client = reqwest::Client::new();
    let body = FileSearchResponseRequest {
        request_id,
        matches,
        error,
    };
    let res = client
        .post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;
    if res.status().is_success() {
        Ok(())
    } else if res.status() == reqwest::StatusCode::NOT_FOUND {
        anyhow::bail!(
            "relayer returned 404 — request expired (web client likely timed out before executor finished)"
        )
    } else {
        anyhow::bail!("relayer returned {}", res.status())
    }
}

/// POST file read response to relayer.
async fn post_file_read_response(
    base_url: &str,
    api_key: &str,
    request_id: Uuid,
    content: Option<String>,
    error: Option<String>,
) -> Result<()> {
    let http_url = base_url
        .replace("wss://", "https://")
        .replace("ws://", "http://");
    let url = format!(
        "{}/api/files/read/response",
        http_url.trim_end_matches("/ws")
    );
    let client = reqwest::Client::new();
    let body = FileReadResponseRequest {
        request_id,
        content,
        error,
    };
    let res = client
        .post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;
    if res.status().is_success() {
        Ok(())
    } else {
        anyhow::bail!("relayer returned {}", res.status())
    }
}

/// List directories under ~/repos/ and return paths like ~/repos/dirname.
fn list_repos_dirs() -> Vec<String> {
    let expanded = shellexpand::tilde("~/repos");
    let path = Path::new(expanded.as_ref());
    let mut paths = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    paths.push(format!("~/repos/{}", name));
                }
            }
        }
    }
    paths.sort();
    paths
}

/// Run `agent models` and parse model IDs.
fn list_agent_models() -> Vec<String> {
    let output = std::process::Command::new("agent")
        .args(["models"])
        .output();
    let Ok(out) = output else {
        return vec!["composer-1.5".to_string()];
    };
    let Ok(stdout) = std::str::from_utf8(&out.stdout) else {
        return vec!["composer-1.5".to_string()];
    };
    let mut models = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("Available") || line.starts_with("Tip:") {
            continue;
        }
        if let Some((id, _)) = line.split_once(" - ") {
            let id = id.trim();
            if !id.is_empty() {
                models.push(id.to_string());
            }
        }
    }
    if models.is_empty() {
        models.push("composer-1.5".to_string());
    }
    models
}

/// Sync model list to relayer. Call on startup.
async fn sync_models_to_relayer(base_url: &str, executor_api_key: &str) -> Result<()> {
    let models = list_agent_models();
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/api/models", base_url))
        .bearer_auth(executor_api_key)
        .json(&serde_json::json!({ "models": models }))
        .send()
        .await?;
    if res.status().is_success() {
        tracing::info!("Synced {} models to relayer", models.len());
    } else {
        tracing::warn!("Models sync failed: {}", res.status());
    }
    Ok(())
}

/// Sync repo list to relayer. Call on startup. Returns local paths for default-repo fallback.
async fn sync_repos_to_relayer(base_url: &str, executor_api_key: &str) -> Result<Vec<String>> {
    let paths = list_repos_dirs();
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/api/repos/sync", base_url))
        .bearer_auth(executor_api_key)
        .json(&serde_json::json!({ "paths": paths }))
        .send()
        .await?;
    if res.status().is_success() {
        tracing::info!("Synced {} repos to relayer", paths.len());
    } else {
        tracing::warn!("Repos sync failed: {}", res.status());
    }
    Ok(paths)
}

pub async fn run_ws_client(
    ws_url: &str,
    executor_api_key: &str,
    default_repo: &str,
    default_translator_model: &str,
    default_workload_model: &str,
) -> Result<()> {
    let http_url = ws_url
        .replace("wss://", "https://")
        .replace("ws://", "http://");
    let base_url = http_url.trim_end_matches("/ws");
    sync_models_to_relayer(base_url, executor_api_key)
        .await
        .ok();
    let local_repos = sync_repos_to_relayer(base_url, executor_api_key)
        .await
        .unwrap_or_default();
    let fallback_repo = local_repos
        .first()
        .map(String::as_str)
        .unwrap_or(default_repo);

    let url = ws_url.to_string();
    loop {
        match connect_async(&url).await {
            Ok((ws, _)) => {
                tracing::info!("Connected to relayer");
                if let Err(e) = handle_connection(
                    ws,
                    fallback_repo,
                    default_translator_model,
                    default_workload_model,
                    executor_api_key,
                    ws_url,
                )
                .await
                {
                    tracing::warn!("Connection error: {}", e);
                }
            }
            Err(e) => {
                tracing::warn!("Connect failed: {}, retrying in 5s", e);
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}

async fn handle_connection(
    ws: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    default_repo: &str,
    default_translator_model: &str,
    default_workload_model: &str,
    executor_api_key: &str,
    base_url: &str,
) -> Result<()> {
    let (mut ws_tx, mut ws_rx) = ws.split();

    // Send auth as first message
    ws_tx
        .send(Message::Text(
            serde_json::json!({
                "type": shared::ws_types::AUTH,
                "payload": { "token": executor_api_key }
            })
            .to_string(),
        ))
        .await?;

    // Wait for auth_ok before processing commands
    let mut authenticated = false;
    while let Some(msg) = ws_rx.next().await {
        let msg = match msg {
            Ok(Message::Text(t)) => t,
            Ok(Message::Close(_)) => break,
            Err(e) => return Err(e.into()),
            _ => continue,
        };

        let envelope: WsEnvelope = match serde_json::from_str(&msg) {
            Ok(e) => e,
            Err(_) => continue,
        };

        if envelope.r#type == shared::ws_types::AUTH_OK {
            authenticated = true;
            continue;
        }
        if envelope.r#type == shared::ws_types::AUTH_FAIL {
            tracing::warn!("WebSocket auth failed");
            break;
        }
        if !authenticated {
            continue;
        }

        if envelope.r#type == shared::ws_types::COMMAND_NEW {
            if let Ok(cmd) =
                serde_json::from_value::<shared::WsCommandNewPayload>(envelope.payload.clone())
            {
                tokio::spawn({
                    let base_url = base_url.to_string();
                    let api_key = executor_api_key.to_string();
                    let repo = cmd
                        .repo_path
                        .clone()
                        .unwrap_or_else(|| default_repo.to_string());
                    let trans = cmd
                        .translator_model
                        .clone()
                        .unwrap_or_else(|| default_translator_model.to_string());
                    let work = cmd
                        .workload_model
                        .clone()
                        .unwrap_or_else(|| default_workload_model.to_string());
                    async move {
                        if let Err(e) =
                            run_command(&base_url, &api_key, cmd, &repo, &trans, &work).await
                        {
                            tracing::error!("Command failed: {}", e);
                        }
                    }
                });
            }
        }
        if envelope.r#type == shared::ws_types::FILE_READ_REQUEST {
            if let Ok(req) =
                serde_json::from_value::<WsFileReadRequestPayload>(envelope.payload.clone())
            {
                tokio::spawn({
                    let base_url = base_url.to_string();
                    let api_key = executor_api_key.to_string();
                    async move {
                        if let Err(e) = handle_file_read(
                            &base_url,
                            &api_key,
                            &req.repo_path,
                            &req.file_path,
                            req.request_id,
                        )
                        .await
                        {
                            tracing::error!("File read failed: {}", e);
                            let _ = post_file_read_response(
                                &base_url,
                                &api_key,
                                req.request_id,
                                None,
                                Some(e.to_string()),
                            )
                            .await;
                        }
                    }
                });
            }
        }
        if envelope.r#type == shared::ws_types::FILE_SEARCH_REQUEST {
            if let Ok(req) =
                serde_json::from_value::<WsFileSearchRequestPayload>(envelope.payload.clone())
            {
                tokio::spawn({
                    let base_url = base_url.to_string();
                    let api_key = executor_api_key.to_string();
                    async move {
                        if let Err(e) = handle_file_search(
                            &base_url,
                            &api_key,
                            &req.repo_path,
                            &req.file_name,
                            req.request_id,
                        )
                        .await
                        {
                            tracing::error!("File search failed: {}", e);
                            let _ = post_file_search_response(
                                &base_url,
                                &api_key,
                                req.request_id,
                                None,
                                Some(e.to_string()),
                            )
                            .await;
                        }
                    }
                });
            }
        }
    }

    Ok(())
}

async fn run_command(
    base_url: &str,
    api_key: &str,
    cmd: WsCommandNewPayload,
    default_repo: &str,
    translator_model: &str,
    workload_model: &str,
) -> Result<()> {
    let http_url = base_url
        .replace("wss://", "https://")
        .replace("ws://", "http://");
    let cmd_url = http_url.trim_end_matches("/ws");
    let patch_url = format!("{}/api/commands/{}", cmd_url, cmd.id);

    let client = reqwest::Client::new();

    // PATCH status = running
    let _ = client
        .patch(&patch_url)
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "status": "running"
        }))
        .send()
        .await;

    let repo = cmd.repo_path.as_deref().unwrap_or(default_repo);
    let trans = cmd.translator_model.as_deref().unwrap_or(translator_model);
    let work = cmd.workload_model.as_deref().unwrap_or(workload_model);

    let patch_url_for_cb = patch_url.to_string();
    let api_key_for_cb = api_key.to_string();
    let last_send = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let throttle_ms = 300u64;
    let on_output = std::sync::Arc::new(move |output: &str| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last = last_send.load(std::sync::atomic::Ordering::Relaxed);
        if last > 0 && now.saturating_sub(last) < throttle_ms {
            return;
        }
        last_send.store(now, std::sync::atomic::Ordering::Relaxed);
        let url = patch_url_for_cb.clone();
        let key = api_key_for_cb.clone();
        let out = output.to_string();
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let _ = client
                .patch(&url)
                .bearer_auth(&key)
                .json(&serde_json::json!({
                    "status": "running",
                    "output": out
                }))
                .send()
                .await;
        });
    });

    let chat_history: Option<Vec<(String, Option<String>)>> = cmd.chat_history.as_ref().map(|v| {
        v.iter()
            .map(|e| (e.input.clone(), e.output.clone()))
            .collect()
    });
    let chat_history_ref = chat_history.as_deref();

    tracing::info!(cmd_id = %cmd.id, "running command");
    let result = cursor::run_command(
        &cmd.input,
        repo,
        trans,
        work,
        cmd.cursor_chat_id.as_deref(),
        Some(on_output),
        cmd.context_mode.as_deref(),
        chat_history_ref,
    )
    .await;

    let (status, output, summary, cursor_chat_id) = match result {
        Ok((out, sum, chat_id)) => ("done", out, sum, Some(chat_id)),
        Err(e) => {
            tracing::error!(err = %e, "command failed");
            ("failed", format!("Error: {}", e), String::new(), None)
        }
    };

    tracing::info!(cmd_id = %cmd.id, status = status, "command finished");
    let mut patch_body = serde_json::json!({
        "status": status,
        "output": output,
        "summary": summary
    });
    if let Some(ref chat_id) = cursor_chat_id {
        patch_body["cursor_chat_id"] = serde_json::json!(chat_id);
    }
    let _ = client
        .patch(&patch_url)
        .bearer_auth(api_key)
        .json(&patch_body)
        .send()
        .await;

    Ok(())
}
