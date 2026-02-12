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
    pub jwt_refresh_grace_secs: u64,
    pub executor_api_key: String,
    pub device_registration_code_ttl_secs: u64,
    pub password_salt: String,
    /// Allowed CORS origins (e.g. frontend URL). Comma-separated in env.
    pub cors_allowed_origins: Vec<String>,
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
        let jwt_refresh_grace_secs = std::env::var("JWT_REFRESH_GRACE_SECS")
            .unwrap_or_else(|_| "86400".to_string())
            .parse()
            .unwrap_or(86400);
        let executor_api_key =
            std::env::var("EXECUTOR_API_KEY").map_err(|_| std::env::VarError::NotPresent)?;
        let device_registration_code_ttl_secs = std::env::var("DEVICE_REGISTRATION_CODE_TTL_SECS")
            .unwrap_or_else(|_| "600".to_string())
            .parse()
            .unwrap_or(600);
        let password_salt =
            std::env::var("PASSWORD_SALT").map_err(|_| std::env::VarError::NotPresent)?;
        let cors_allowed_origins = std::env::var("CORS_ALLOWED_ORIGINS")
            .map(|s| {
                s.split(',')
                    .map(|o| o.trim().to_string())
                    .filter(|o| !o.is_empty())
                    .collect()
            })
            .unwrap_or_else(|_| {
                vec![
                    "http://localhost:5173".to_string(),
                    "http://127.0.0.1:5173".to_string(),
                ]
            });

        Ok(Self {
            host,
            port,
            database_path,
            jwt_secret,
            jwt_ttl_secs,
            jwt_refresh_grace_secs,
            executor_api_key,
            device_registration_code_ttl_secs,
            password_salt,
            cors_allowed_origins,
        })
    }

    /// Build config for tests without reading env. Avoids env var races in parallel tests.
    #[cfg(test)]
    pub fn for_test(
        database_path: PathBuf,
        jwt_secret: impl Into<String>,
        executor_api_key: impl Into<String>,
        password_salt: impl Into<String>,
    ) -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            database_path,
            jwt_secret: jwt_secret.into(),
            jwt_ttl_secs: 3600,
            jwt_refresh_grace_secs: 86400,
            executor_api_key: executor_api_key.into(),
            device_registration_code_ttl_secs: 600,
            password_salt: password_salt.into(),
            cors_allowed_origins: vec!["http://localhost:5173".to_string()],
        }
    }
}
