//! Configuration for the relayer.

use std::path::PathBuf;

/// Relayer configuration.
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_path: PathBuf,
    pub jwt_secret: String,
    pub jwt_ttl_secs: u64,
    pub executor_api_key: String,
    pub device_registration_code_ttl_secs: u64,
}

impl Config {
    pub fn from_env() -> Result<Self, std::env::VarError> {
        let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port: u16 = std::env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .unwrap_or(8080);
        let database_path = std::env::var("DATABASE_PATH")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .map(|s| PathBuf::from(s.trim_start_matches("sqlite:")))
            .unwrap_or_else(|_| PathBuf::from("./data/relayer.db"));
        let jwt_secret = std::env::var("JWT_SECRET").map_err(|_| std::env::VarError::NotPresent)?;
        let jwt_ttl_secs = std::env::var("JWT_TTL_SECS")
            .unwrap_or_else(|_| "3600".to_string())
            .parse()
            .unwrap_or(3600);
        let executor_api_key =
            std::env::var("EXECUTOR_API_KEY").map_err(|_| std::env::VarError::NotPresent)?;
        let device_registration_code_ttl_secs = std::env::var("DEVICE_REGISTRATION_CODE_TTL_SECS")
            .unwrap_or_else(|_| "600".to_string())
            .parse()
            .unwrap_or(600);

        Ok(Self {
            host,
            port,
            database_path,
            jwt_secret,
            jwt_ttl_secs,
            executor_api_key,
            device_registration_code_ttl_secs,
        })
    }
}
