//! Dev PM Agent Executor — desktop daemon.

use std::env;

use clap::Parser;
use executor::{cli, relay_client};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = cli::Cli::parse();

    match cli.command.unwrap_or(cli::Commands::Run) {
        cli::Commands::Run => {
            let ws_url =
                env::var("RELAYER_WS_URL").unwrap_or_else(|_| "ws://localhost:8080/ws".to_string());
            let api_key = env::var("EXECUTOR_API_KEY")
                .map_err(|_| anyhow::anyhow!("EXECUTOR_API_KEY required"))?;
            let default_repo =
                env::var("DEFAULT_REPO").unwrap_or_else(|_| "~/repos/default".to_string());
            let translator_model =
                env::var("TRANSLATOR_MODEL").unwrap_or_else(|_| "composer-1.5".to_string());
            let workload_model =
                env::var("WORKLOAD_MODEL").unwrap_or_else(|_| "composer-1.5".to_string());

            relay_client::run_ws_client(
                &ws_url,
                &api_key,
                &default_repo,
                &translator_model,
                &workload_model,
            )
            .await?;
        }
        cli::Commands::RegisterDevice { code, password } => {
            let relayer_url =
                env::var("RELAYER_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
            let api_key = env::var("EXECUTOR_API_KEY")
                .map_err(|_| anyhow::anyhow!("EXECUTOR_API_KEY required"))?;

            let client = reqwest::Client::new();
            let res = client
                .post(format!("{}/api/auth/register-device", relayer_url))
                .bearer_auth(&api_key)
                .json(&serde_json::json!({ "code": code, "password": password }))
                .send()
                .await?;

            if !res.status().is_success() {
                let err: String = res.text().await.unwrap_or_default();
                anyhow::bail!("Registration failed: {}", err);
            }

            let body: serde_json::Value = res.json().await?;
            let device_api_key = body["device_api_key"].as_str().unwrap_or("");
            let totp_secret = body["totp_secret"].as_str().unwrap_or("");

            println!("Device registered successfully.");
            println!();
            println!("Device API key (save for login):");
            println!("{}", device_api_key);
            println!();
            println!("Add this TOTP secret to your authenticator app:");
            println!("{}", totp_secret);
        }
    }

    Ok(())
}
