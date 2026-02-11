//! WebSocket client for receiving commands from relayer.

use anyhow::Result;
use futures_util::StreamExt;
use shared::{WsCommandNewPayload, WsEnvelope};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::cursor;

pub async fn run_ws_client(
    ws_url: &str,
    executor_api_key: &str,
    default_repo: &str,
    default_translator_model: &str,
    default_workload_model: &str,
) -> Result<()> {
    let url = format!("{}?token={}", ws_url, executor_api_key);
    loop {
        match connect_async(&url).await {
            Ok((ws, _)) => {
                tracing::info!("Connected to relayer");
                if let Err(e) = handle_connection(
                    ws,
                    default_repo,
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
    let (_ws_tx, mut ws_rx) = ws.split();

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

    let result = cursor::run_command(&cmd.input, repo, trans, work).await;

    let (status, output, summary) = match result {
        Ok((out, sum)) => ("done", out, sum),
        Err(e) => ("failed", format!("Error: {}", e), String::new()),
    };

    let _ = client
        .patch(&patch_url)
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "status": status,
            "output": output,
            "summary": summary
        }))
        .send()
        .await;

    Ok(())
}
