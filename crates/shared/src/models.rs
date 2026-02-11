//! Shared request/response and domain models.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Device role in the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceRole {
    Executor,
    Controller,
}

/// Command status in the relay pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandStatus {
    Pending,
    Running,
    Done,
    Failed,
    Cancelled,
}

impl CommandStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Create command request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCommandRequest {
    pub input: String,
    pub repo_path: Option<String>,
    pub context_mode: Option<String>,
    pub translator_model: Option<String>,
    pub workload_model: Option<String>,
    /// When set, executor resumes this Cursor chat instead of creating a new one.
    pub cursor_chat_id: Option<String>,
}

/// Command response (full details).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse {
    pub id: Uuid,
    pub device_id: Uuid,
    pub input: String,
    pub status: CommandStatus,
    pub output: Option<String>,
    pub summary: Option<String>,
    pub repo_path: Option<String>,
    pub context_mode: Option<String>,
    pub translator_model: Option<String>,
    pub workload_model: Option<String>,
    pub cursor_chat_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Update command request (executor).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCommandRequest {
    pub status: Option<CommandStatus>,
    pub output: Option<String>,
    pub summary: Option<String>,
    pub cursor_chat_id: Option<String>,
}

// --- Auth DTOs ---

/// Setup request (first-run only).
/// device_api_key must have been registered via CLI (bootstrap-device) first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupRequest {
    pub device_api_key: String,
    pub username: String,
    pub password: String,
}

/// Setup response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupResponse {
    pub totp_secret: String,
}

/// Login request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub device_api_key: String,
    pub password: String,
    pub totp_code: String,
}

/// Login response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
}

/// Refresh token request (old JWT may be expired but within grace period).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshRequest {
    pub token: String,
}

/// Refresh token response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshResponse {
    pub token: String,
}

/// Reserve code request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReserveCodeRequest {
    pub code: String,
}

/// Reserve code response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReserveCodeResponse {
    pub expires_at: String,
}

/// Verify bootstrap device request (web setup step 1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyBootstrapRequest {
    pub device_api_key: String,
}

/// Verify bootstrap response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyBootstrapResponse {
    pub valid: bool,
}

/// Bootstrap device response (CLI gets device key from this).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapDeviceResponse {
    pub device_api_key: String,
}

/// Register device request (executor CLI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDeviceRequest {
    pub code: String,
    pub password: String,
}

/// Register device response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDeviceResponse {
    pub device_api_key: String,
    pub totp_secret: String,
}

// --- WebSocket envelope ---

/// WebSocket message envelope (version 1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsEnvelope {
    #[serde(default)]
    pub version: u8,
    pub r#type: String,
    pub payload: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
}

/// WebSocket message types.
pub mod ws_types {
    pub const AUTH: &str = "auth";
    pub const AUTH_OK: &str = "auth_ok";
    pub const AUTH_FAIL: &str = "auth_fail";
    pub const COMMAND_NEW: &str = "command_new";
    pub const COMMAND_UPDATE: &str = "command_update";
    pub const COMMAND_ACK: &str = "command_ack";
    pub const COMMAND_RESULT: &str = "command_result";
    pub const FILE_READ_REQUEST: &str = "file_read_request";
    pub const FILE_SEARCH_REQUEST: &str = "file_search_request";
    pub const PING: &str = "ping";
    pub const PONG: &str = "pong";
    pub const ERROR: &str = "error";
}

/// Auth message payload (client → server).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsAuthPayload {
    pub token: String,
}

/// Single turn in chat history (user input + assistant output).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatHistoryEntry {
    pub input: String,
    pub output: Option<String>,
}

/// command_new payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsCommandNewPayload {
    pub id: Uuid,
    pub input: String,
    pub repo_path: Option<String>,
    pub context_mode: Option<String>,
    pub translator_model: Option<String>,
    pub workload_model: Option<String>,
    /// Resume this Cursor chat (skip create-chat when set).
    pub cursor_chat_id: Option<String>,
    /// Prior turns in this chat (for translator context). Present when resuming.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_history: Option<Vec<ChatHistoryEntry>>,
}

/// command_update payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsCommandUpdatePayload {
    pub id: Uuid,
    pub status: String,
    pub output: Option<String>,
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_chat_id: Option<String>,
    pub updated_at: String,
}

/// command_ack payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsCommandAckPayload {
    pub id: Uuid,
}

/// command_result payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsCommandResultPayload {
    pub id: Uuid,
    pub status: String, // "done" | "failed"
    pub output: String,
    pub summary: String,
}

/// file_read_request payload (relayer → executor).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsFileReadRequestPayload {
    pub request_id: Uuid,
    pub repo_path: String,
    pub file_path: String,
}

/// File read response (executor → relayer via HTTP POST).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadResponseRequest {
    pub request_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// file_search_request payload (relayer → executor).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsFileSearchRequestPayload {
    pub request_id: Uuid,
    pub repo_path: String,
    pub file_name: String,
}

/// Single file search match: path relative to repo root, modified timestamp (ISO8601).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchMatch {
    pub path: String,
    pub modified_at: String,
}

/// File search response (executor → relayer via HTTP POST).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchResponseRequest {
    pub request_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matches: Option<Vec<FileSearchMatch>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Add repo request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddRepoRequest {
    pub path: String,
    pub name: Option<String>,
}

/// Sync repos request (executor). Replaces admin's repos with listed paths from ~/repos/.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReposRequest {
    pub paths: Vec<String>,
}

/// Sync models request (executor). Replaces cached model list from `agent models`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncModelsRequest {
    pub models: Vec<String>,
}

/// Repo response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoResponse {
    pub id: Uuid,
    pub path: String,
    pub name: Option<String>,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ws_types;
    use serde_json;

    fn random_uuid() -> Uuid {
        Uuid::new_v4()
    }

    #[test]
    fn create_command_request_serde_roundtrip() {
        let req = CreateCommandRequest {
            input: "add tests".to_string(),
            repo_path: Some("~/repos/foo".to_string()),
            context_mode: Some("continue".to_string()),
            translator_model: Some("claude-4".to_string()),
            workload_model: Some("cursor".to_string()),
            cursor_chat_id: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: CreateCommandRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.input, req.input);
        assert_eq!(parsed.repo_path, req.repo_path);
    }

    #[test]
    fn command_response_serde_roundtrip() {
        let id = random_uuid();
        let device_id = random_uuid();
        let resp = CommandResponse {
            id,
            device_id,
            input: "test".to_string(),
            status: CommandStatus::Done,
            output: Some("done".to_string()),
            summary: Some("OK".to_string()),
            repo_path: None,
            context_mode: None,
            translator_model: None,
            workload_model: None,
            cursor_chat_id: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:01Z".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: CommandResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, id);
        assert_eq!(parsed.status, CommandStatus::Done);
    }

    #[test]
    fn ws_envelope_serde_roundtrip() {
        let env = WsEnvelope {
            version: 1,
            r#type: ws_types::COMMAND_NEW.to_string(),
            payload: serde_json::json!({"id": random_uuid().to_string(), "input": "hi"}),
            ts: Some("2025-01-01T00:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&env).unwrap();
        let parsed: WsEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.r#type, ws_types::COMMAND_NEW);
    }

    #[test]
    fn setup_response_serde_roundtrip() {
        let resp = SetupResponse {
            totp_secret: "JBSWY3DPEHPK3PXP".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: SetupResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.totp_secret, resp.totp_secret);
    }

    #[test]
    fn device_role_serde() {
        let r = DeviceRole::Controller;
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("controller"));
        let parsed: DeviceRole = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeviceRole::Controller);
    }
}
