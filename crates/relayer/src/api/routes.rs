//! API route handlers.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use uuid::Uuid;

use shared::{
    AddRepoRequest, CreateCommandRequest, LoginRequest, LoginResponse, RegisterDeviceRequest,
    RegisterDeviceResponse, ReserveCodeRequest, ReserveCodeResponse, SetupRequest, SetupResponse,
    UpdateCommandRequest,
};
use shared::{CommandResponse, CommandStatus, RepoResponse};

use crate::api::AppState;
use crate::auth::{create_jwt, generate_api_key, generate_totp_secret, hash_api_key, verify_totp};
use crate::db;
use crate::relay::BroadcastMessage;

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/setup", post(auth_setup))
        .route("/auth/login", post(auth_login))
        .route("/auth/register-device", post(auth_register_device))
        .route("/devices/reserve-code", post(devices_reserve_code))
        .route("/commands", post(commands_create).get(commands_list))
        .route("/commands/:id", get(commands_get).patch(commands_update))
        .route("/repos", get(repos_list).post(repos_add))
        .route("/models", get(models_list))
}

// --- Auth ---

async fn auth_setup(
    State(state): State<AppState>,
    Json(req): Json<SetupRequest>,
) -> Result<Json<SetupResponse>, (StatusCode, String)> {
    let conn = state.db.0.lock().unwrap();
    if db::admin_exists(&conn).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))? {
        return Err((StatusCode::FORBIDDEN, "setup already completed".to_string()));
    }
    let password_hash = bcrypt::hash(&req.password, bcrypt::DEFAULT_COST)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let totp_secret =
        generate_totp_secret().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let device_api_key = generate_api_key();
    let device_api_key_hash = hash_api_key(&device_api_key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db::setup_admin(
        &conn,
        &req.username,
        &password_hash,
        &totp_secret,
        &device_api_key_hash,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(SetupResponse {
        totp_secret,
        device_api_key,
    }))
}

async fn auth_login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    let api_key_hash = hash_api_key(&req.device_api_key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let conn = state.db.0.lock().unwrap();
    let Some((device_id, admin_id, _role)) = db::validate_device(&conn, &api_key_hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    else {
        return Err((StatusCode::UNAUTHORIZED, "invalid device".to_string()));
    };
    let (_, password_hash, totp_secret): (String, String, String) = conn
        .query_row(
            "SELECT id, password_hash, totp_secret FROM admin WHERE id = ?1",
            [admin_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid credentials".to_string()))?;
    if !bcrypt::verify(&req.password, &password_hash).unwrap_or(false) {
        return Err((StatusCode::UNAUTHORIZED, "invalid credentials".to_string()));
    }
    if !verify_totp(&totp_secret, &req.totp_code) {
        return Err((StatusCode::UNAUTHORIZED, "invalid totp".to_string()));
    }
    let token = create_jwt(
        device_id,
        admin_id,
        "controller",
        &state.config.jwt_secret,
        state.config.jwt_ttl_secs as u64,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(LoginResponse { token }))
}

async fn auth_register_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RegisterDeviceRequest>,
) -> Result<Json<RegisterDeviceResponse>, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    if token != state.config.executor_api_key {
        return Err((
            StatusCode::UNAUTHORIZED,
            "invalid executor api key".to_string(),
        ));
    }
    let device_api_key = generate_api_key();
    let device_api_key_hash = hash_api_key(&device_api_key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let conn = state.db.0.lock().unwrap();
    let Some(totp_secret) =
        db::register_device(&conn, &req.code, &req.password, &device_api_key_hash)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    else {
        return Err((
            StatusCode::BAD_REQUEST,
            "invalid code or password".to_string(),
        ));
    };
    Ok(Json(RegisterDeviceResponse {
        device_api_key,
        totp_secret,
    }))
}

// --- Devices ---

async fn devices_reserve_code(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ReserveCodeRequest>,
) -> Result<Json<ReserveCodeResponse>, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    let (device_id, _admin_id, _) = verify_bearer(&token, &state)?;
    let conn = state.db.0.lock().unwrap();
    let expires_at = chrono::Utc::now()
        + chrono::Duration::seconds(state.config.device_registration_code_ttl_secs as i64);
    let expires_at_str = expires_at.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    db::reserve_code(&conn, &req.code, device_id, &expires_at_str)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ReserveCodeResponse {
        expires_at: expires_at_str,
    }))
}

// --- Commands ---

async fn commands_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateCommandRequest>,
) -> Result<Json<CommandResponse>, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    let (device_id, _admin_id, _) = verify_bearer(&token, &state)?;
    if req.input.len() > 4096 {
        return Err((StatusCode::BAD_REQUEST, "input too long".to_string()));
    }
    let conn = state.db.0.lock().unwrap();
    let id = db::create_command(
        &conn,
        device_id,
        &req.input,
        req.repo_path.as_deref(),
        req.context_mode.as_deref(),
        req.translator_model.as_deref(),
        req.workload_model.as_deref(),
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let cmd = db::get_command(&conn, id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "command not found".to_string(),
        ))?;
    let status = match cmd.2.as_str() {
        "pending" => CommandStatus::Pending,
        "running" => CommandStatus::Running,
        "done" => CommandStatus::Done,
        "failed" => CommandStatus::Failed,
        _ => CommandStatus::Pending,
    };
    let response = CommandResponse {
        id: cmd.0,
        device_id: cmd.1,
        input: cmd.2.clone(),
        status,
        output: cmd.4.clone(),
        summary: cmd.5.clone(),
        repo_path: cmd.6.clone(),
        context_mode: cmd.7.clone(),
        translator_model: cmd.8.clone(),
        workload_model: cmd.9.clone(),
        created_at: cmd.10.clone(),
        updated_at: cmd.11.clone(),
    };
    // Broadcast to executor
    state
        .relay
        .broadcast(BroadcastMessage::CommandNew(shared::WsCommandNewPayload {
            id,
            input: req.input,
            repo_path: req.repo_path,
            context_mode: req.context_mode,
            translator_model: req.translator_model,
            workload_model: req.workload_model,
        }));
    Ok(Json(response))
}

async fn commands_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CommandResponse>>, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    let (_, admin_id, _) = verify_bearer(&token, &state)?;
    let conn = state.db.0.lock().unwrap();
    let cmds = db::list_commands(&conn, admin_id, 100)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let responses: Vec<CommandResponse> = cmds
        .into_iter()
        .map(|c| {
            let status = match c.3.as_str() {
                "pending" => CommandStatus::Pending,
                "running" => CommandStatus::Running,
                "done" => CommandStatus::Done,
                "failed" => CommandStatus::Failed,
                _ => CommandStatus::Pending,
            };
            CommandResponse {
                id: c.0,
                device_id: c.1,
                input: c.2,
                status,
                output: c.4,
                summary: c.5,
                repo_path: c.6,
                context_mode: c.7,
                translator_model: c.8,
                workload_model: c.9,
                created_at: c.10,
                updated_at: c.11,
            }
        })
        .collect();
    Ok(Json(responses))
}

async fn commands_get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<CommandResponse>, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    let (_, _admin_id, _) = verify_bearer(&token, &state)?;
    let conn = state.db.0.lock().unwrap();
    let Some(cmd) = db::get_command(&conn, id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    else {
        return Err((StatusCode::NOT_FOUND, "command not found".to_string()));
    };
    // TODO: verify admin_id matches
    let status = match cmd.3.as_str() {
        "pending" => CommandStatus::Pending,
        "running" => CommandStatus::Running,
        "done" => CommandStatus::Done,
        "failed" => CommandStatus::Failed,
        _ => CommandStatus::Pending,
    };
    Ok(Json(CommandResponse {
        id: cmd.0,
        device_id: cmd.1,
        input: cmd.2,
        status,
        output: cmd.4,
        summary: cmd.5,
        repo_path: cmd.6,
        context_mode: cmd.7,
        translator_model: cmd.8,
        workload_model: cmd.9,
        created_at: cmd.10,
        updated_at: cmd.11,
    }))
}

async fn commands_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateCommandRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    let (_, admin_id, _) = verify_bearer(&token, &state)?;
    let conn = state.db.0.lock().unwrap();
    let status = req.status.as_ref().map(|s| s.as_str());
    let _ = admin_id;
    db::update_command(
        &conn,
        id,
        status,
        req.output.as_deref(),
        req.summary.as_deref(),
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    state.relay.broadcast(BroadcastMessage::CommandUpdate(
        shared::WsCommandUpdatePayload {
            id,
            status: status.unwrap_or("").to_string(),
            output: req.output,
            summary: req.summary,
            updated_at: now,
        },
    ));
    Ok(StatusCode::NO_CONTENT)
}

// --- Models ---

/// Static model list. Can later be replaced with executor query or config.
const MODELS: &[&str] = &["composer-1.5", "claude-4", "claude-3.5-sonnet", "gpt-4o"];

async fn models_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    let _ = verify_bearer(&token, &state)?;
    Ok(Json(MODELS.iter().map(|s| (*s).to_string()).collect()))
}

// --- Repos ---

async fn repos_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RepoResponse>>, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    let (_, admin_id, _) = verify_bearer(&token, &state)?;
    let conn = state.db.0.lock().unwrap();
    let repos = db::list_repos(&conn, admin_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let responses: Vec<RepoResponse> = repos
        .into_iter()
        .map(|r| RepoResponse {
            id: r.0,
            path: r.1,
            name: r.2,
            created_at: r.3,
        })
        .collect();
    Ok(Json(responses))
}

async fn repos_add(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AddRepoRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    let (_, admin_id, _) = verify_bearer(&token, &state)?;
    let conn = state.db.0.lock().unwrap();
    db::add_repo(&conn, admin_id, &req.path, req.name.as_deref())
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(StatusCode::CREATED)
}

// --- WebSocket ---

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    token: Option<String>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(q): Query<WsQuery>,
    State(state): State<AppState>,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_socket(socket, q.token, state))
}

async fn handle_socket(socket: WebSocket, token: Option<String>, state: AppState) {
    let token = token.filter(|t| !t.is_empty());
    if token.is_none() {
        return;
    }
    let token = token.unwrap();
    // Validate: JWT (controller) or EXECUTOR_API_KEY (executor)
    let (_device_id, _admin_id, _role) = if token == state.config.executor_api_key {
        // Executor - we need admin_id. For single-admin, we get from first admin.
        let conn = state.db.0.lock().unwrap();
        let admin_id: Option<String> = conn
            .query_row("SELECT id FROM admin LIMIT 1", [], |row| row.get(0))
            .ok();
        match admin_id {
            Some(a) => (
                Uuid::nil(),
                Uuid::parse_str(&a).unwrap_or(Uuid::nil()),
                "executor".to_string(),
            ),
            None => return,
        }
    } else {
        match crate::auth::validate_jwt(&token, &state.config.jwt_secret) {
            Ok(Some((d, a, r))) => (d, a, r),
            _ => return,
        }
    };
    let mut rx = state.relay.subscribe();
    let (mut ws_tx, mut ws_rx) = socket.split();
    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let json = match &msg {
                BroadcastMessage::CommandNew(p) => serde_json::to_string(&shared::WsEnvelope {
                    version: 1,
                    r#type: shared::ws_types::COMMAND_NEW.to_string(),
                    payload: serde_json::to_value(p).unwrap(),
                    ts: Some(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()),
                }),
                BroadcastMessage::CommandUpdate(p) => serde_json::to_string(&shared::WsEnvelope {
                    version: 1,
                    r#type: shared::ws_types::COMMAND_UPDATE.to_string(),
                    payload: serde_json::to_value(p).unwrap(),
                    ts: Some(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()),
                }),
            };
            if let Ok(j) = json {
                let _ = ws_tx.send(Message::Text(j)).await;
            }
        }
    });
    while let Some(_) = ws_rx.next().await {}
}

// --- Auth ---

fn extract_bearer_from_headers(headers: &HeaderMap) -> Result<String, (StatusCode, String)> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").map(String::from))
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "missing authorization".to_string(),
        ))
}

fn verify_bearer(
    token: &str,
    state: &AppState,
) -> Result<(Uuid, Uuid, String), (StatusCode, String)> {
    if token == state.config.executor_api_key {
        let conn = state.db.0.lock().unwrap();
        let admin_id: String = conn
            .query_row("SELECT id FROM admin LIMIT 1", [], |row| row.get(0))
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok((
            Uuid::nil(),
            Uuid::parse_str(&admin_id)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
            "executor".to_string(),
        ));
    }
    crate::auth::validate_jwt(token, &state.config.jwt_secret)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::UNAUTHORIZED, "invalid token".to_string()))
}
