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
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use uuid::Uuid;

use shared::{
    AddRepoRequest, BootstrapDeviceResponse, CreateCommandRequest, FileReadResponseRequest,
    FileSearchResponseRequest, LoginRequest, LoginResponse, RefreshRequest, RefreshResponse,
    RegisterDeviceRequest, RegisterDeviceResponse, ReserveCodeRequest, ReserveCodeResponse,
    SetupRequest, SetupResponse, SyncModelsRequest, SyncReposRequest, UpdateCommandRequest,
    VerifyBootstrapRequest, VerifyBootstrapResponse, WsFileReadRequestPayload,
    WsFileSearchRequestPayload,
};
use shared::{CommandResponse, CommandStatus, RepoResponse};

use crate::api::AppState;
use crate::auth::{
    create_jwt, decode_jwt_ignore_exp, generate_api_key, generate_totp_secret, hash_api_key,
    verify_totp,
};
use crate::db;
use crate::relay::BroadcastMessage;

/// Per-IP rate limit for auth endpoints: 5 requests per burst, 1 replenish every 15 seconds.
/// Mitigates brute-force on passwords, API keys, TOTP codes, and token stuffing.
fn auth_rate_limit_layer() -> GovernorLayer<
    tower_governor::key_extractor::PeerIpKeyExtractor,
    governor::middleware::NoOpMiddleware,
    axum::body::Body,
> {
    let config = GovernorConfigBuilder::default()
        .per_second(15)
        .burst_size(5)
        .finish()
        .expect("invalid governor config");
    GovernorLayer::new(config)
}

pub fn api_routes() -> Router<AppState> {
    let auth_routes = Router::new()
        .route("/auth/bootstrap-device", post(auth_bootstrap_device))
        .route("/auth/verify-bootstrap", post(auth_verify_bootstrap))
        .route("/auth/setup", post(auth_setup))
        .route("/auth/login", post(auth_login))
        .route("/auth/refresh", post(auth_refresh))
        .route("/auth/register-device", post(auth_register_device))
        .layer(auth_rate_limit_layer());

    Router::new()
        .merge(auth_routes)
        .route("/devices/reserve-code", post(devices_reserve_code))
        .route("/commands", post(commands_create).get(commands_list))
        .route(
            "/commands/{id}",
            get(commands_get)
                .patch(commands_update)
                .delete(commands_delete),
        )
        .route("/repos", get(repos_list).post(repos_add))
        .route("/repos/sync", post(repos_sync))
        .route("/models", get(models_list).post(models_sync))
        .route("/files/read", get(files_read))
        .route("/files/read/response", post(files_read_response))
        .route("/files/search", get(files_search))
        .route("/files/search/response", post(files_search_response))
}

// --- Auth ---

/// Bootstrap device: generate device key, store in bootstrap_devices.
/// Requires EXECUTOR_API_KEY. Only when no admin exists.
async fn auth_bootstrap_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(_req): Json<serde_json::Value>,
) -> Result<Json<BootstrapDeviceResponse>, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    if token != state.config.executor_api_key {
        return Err((
            StatusCode::UNAUTHORIZED,
            "invalid executor api key".to_string(),
        ));
    }
    let conn = state.db.0.lock().unwrap();
    if db::admin_exists(&conn).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))? {
        return Err((StatusCode::FORBIDDEN, "setup already completed".to_string()));
    }
    let device_api_key = generate_api_key();
    let device_api_key_hash = hash_api_key(&device_api_key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db::insert_bootstrap_device(&conn, &device_api_key_hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(BootstrapDeviceResponse { device_api_key }))
}

/// Verify bootstrap: check if device key exists in bootstrap_devices.
async fn auth_verify_bootstrap(
    State(state): State<AppState>,
    Json(req): Json<VerifyBootstrapRequest>,
) -> Result<Json<VerifyBootstrapResponse>, (StatusCode, String)> {
    let conn = state.db.0.lock().unwrap();
    if db::admin_exists(&conn).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))? {
        return Err((StatusCode::FORBIDDEN, "setup already completed".to_string()));
    }
    let valid = db::exists_bootstrap_device(&conn, &req.device_api_key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(VerifyBootstrapResponse { valid }))
}

async fn auth_setup(
    State(state): State<AppState>,
    Json(req): Json<SetupRequest>,
) -> Result<Json<SetupResponse>, (StatusCode, String)> {
    let conn = state.db.0.lock().unwrap();
    if db::admin_exists(&conn).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))? {
        return Err((StatusCode::FORBIDDEN, "setup already completed".to_string()));
    }
    if !db::exists_bootstrap_device(&conn, &req.device_api_key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "device key not registered. Run bootstrap-device first.".to_string(),
        ));
    }
    if !db::take_bootstrap_device(&conn, &req.device_api_key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "device key already used".to_string(),
        ));
    }
    let device_api_key_hash = hash_api_key(&req.device_api_key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let salted = format!("{}{}", state.config.password_salt, req.password);
    let password_hash = bcrypt::hash(&salted, bcrypt::DEFAULT_COST)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let totp_secret =
        generate_totp_secret().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db::setup_admin(
        &conn,
        &req.username,
        &password_hash,
        &totp_secret,
        &device_api_key_hash,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(SetupResponse { totp_secret }))
}

async fn auth_login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    let conn = state.db.0.lock().unwrap();
    let Some((device_id, admin_id, _role)) = db::validate_device(&conn, &req.device_api_key)
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
    let salted = format!("{}{}", state.config.password_salt, req.password);
    if !bcrypt::verify(&salted, &password_hash).unwrap_or(false) {
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

/// Refresh JWT. Accepts an expired token if within grace period (default 24h).
async fn auth_refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<RefreshResponse>, (StatusCode, String)> {
    let Some(claims) = decode_jwt_ignore_exp(&req.token, &state.config.jwt_secret)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    else {
        return Err((StatusCode::UNAUTHORIZED, "invalid token".to_string()));
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .as_secs() as i64;
    let grace = state.config.jwt_refresh_grace_secs as i64;
    if claims.exp < now - grace {
        return Err((
            StatusCode::UNAUTHORIZED,
            "token expired beyond refresh window".to_string(),
        ));
    }
    let device_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let admin_id = Uuid::parse_str(&claims.admin_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let token = create_jwt(
        device_id,
        admin_id,
        &claims.role,
        &state.config.jwt_secret,
        state.config.jwt_ttl_secs as u64,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(RefreshResponse { token }))
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
    let Some(totp_secret) = db::register_device(
        &conn,
        &req.code,
        &req.password,
        &device_api_key_hash,
        &state.config.password_salt,
    )
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
        req.cursor_chat_id.as_deref(),
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
        cursor_chat_id: cmd.10.clone(),
        created_at: cmd.11.clone(),
        updated_at: cmd.12.clone(),
    };
    // Fetch chat history when resuming a chat (for translator context)
    let chat_history = if let Some(ref cid) = req.cursor_chat_id {
        db::list_commands_by_cursor_chat_id(&conn, device_id, cid)
            .ok()
            .map(|rows| {
                rows.into_iter()
                    .map(|(input, output)| shared::ChatHistoryEntry { input, output })
                    .collect::<Vec<_>>()
            })
            .filter(|v: &Vec<_>| !v.is_empty())
    } else {
        None
    };

    // Broadcast to executor
    state
        .relay
        .broadcast(BroadcastMessage::CommandNew(shared::WsCommandNewPayload {
            id,
            input: req.input.clone(),
            repo_path: req.repo_path.clone(),
            context_mode: req.context_mode.clone(),
            translator_model: req.translator_model.clone(),
            workload_model: req.workload_model.clone(),
            cursor_chat_id: req.cursor_chat_id.clone(),
            chat_history,
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
                cursor_chat_id: c.10,
                created_at: c.11,
                updated_at: c.12,
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
    // Single-admin design: any authenticated user can access any command (no per-device isolation).
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
        cursor_chat_id: cmd.10,
        created_at: cmd.11,
        updated_at: cmd.12,
    }))
}

/// Update command status/output/summary. EXECUTOR_API_KEY only.
/// Controllers must not update command status—return 403 for JWT tokens.
async fn commands_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateCommandRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    if token != state.config.executor_api_key {
        if crate::auth::validate_jwt(&token, &state.config.jwt_secret)
            .map(|o| o.is_some())
            .unwrap_or(false)
        {
            return Err((
                StatusCode::FORBIDDEN,
                "controller cannot update command status".to_string(),
            ));
        }
        return Err((
            StatusCode::UNAUTHORIZED,
            "invalid executor api key".to_string(),
        ));
    }
    let conn = state.db.0.lock().unwrap();
    let status = req.status.as_ref().map(|s| s.as_str());
    db::update_command(
        &conn,
        id,
        status,
        req.output.as_deref(),
        req.summary.as_deref(),
        req.cursor_chat_id.as_deref(),
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    state.relay.broadcast(BroadcastMessage::CommandUpdate(
        shared::WsCommandUpdatePayload {
            id,
            status: status.unwrap_or("").to_string(),
            output: req.output,
            summary: req.summary,
            cursor_chat_id: req.cursor_chat_id.clone(),
            updated_at: now,
        },
    ));
    Ok(StatusCode::NO_CONTENT)
}

async fn commands_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    let (_, admin_id, _) = verify_bearer(&token, &state)?;
    let conn = state.db.0.lock().unwrap();
    let deleted = db::delete_command(&conn, id, admin_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !deleted {
        return Err((StatusCode::NOT_FOUND, "command not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

// --- Models ---

async fn models_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    let _ = verify_bearer(&token, &state)?;
    let models = state.models.read().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("models lock: {}", e),
        )
    })?;
    Ok(Json(models.clone()))
}

/// Sync models from executor. Requires EXECUTOR_API_KEY. Replaces cached model list.
async fn models_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SyncModelsRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    if token != state.config.executor_api_key {
        return Err((
            StatusCode::UNAUTHORIZED,
            "invalid executor api key".to_string(),
        ));
    }
    if req.models.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "models list must not be empty".to_string(),
        ));
    }
    let mut models = state.models.write().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("models lock: {}", e),
        )
    })?;
    *models = req.models;
    Ok(StatusCode::NO_CONTENT)
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

// --- Files ---

#[derive(serde::Deserialize)]
struct FilesReadQuery {
    repo_path: String,
    file_path: String,
}

#[derive(serde::Deserialize)]
struct FilesSearchQuery {
    repo_path: String,
    file_name: String,
}

/// Read file from repo. Requires JWT (controller). Relayer forwards to executor via WebSocket.
async fn files_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<FilesReadQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let _ = extract_bearer_from_headers(&headers).and_then(|t| verify_bearer(&t, &state))?;
    if q.repo_path.is_empty() || q.file_path.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "repo_path and file_path required".to_string(),
        ));
    }
    let request_id = Uuid::new_v4();
    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut pending = state.file_read_pending.write().unwrap();
        pending.insert(request_id, tx);
    }
    state.relay.broadcast(BroadcastMessage::FileReadRequest(
        WsFileReadRequestPayload {
            request_id,
            repo_path: q.repo_path.clone(),
            file_path: q.file_path.clone(),
        },
    ));
    let result = tokio::time::timeout(std::time::Duration::from_secs(15), rx).await;
    {
        let mut pending = state.file_read_pending.write().unwrap();
        pending.remove(&request_id);
    }
    match result {
        Ok(Ok(Ok(content))) => Ok(Json(serde_json::json!({ "content": content }))),
        Ok(Ok(Err(e))) => Err((StatusCode::BAD_REQUEST, e)),
        Ok(Err(_)) => Err((
            StatusCode::GATEWAY_TIMEOUT,
            "executor did not respond in time".to_string(),
        )),
        Err(_) => Err((
            StatusCode::GATEWAY_TIMEOUT,
            "executor did not respond in time".to_string(),
        )),
    }
}

/// Executor responds with file content. Requires EXECUTOR_API_KEY.
async fn files_read_response(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<FileReadResponseRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    if token != state.config.executor_api_key {
        return Err((
            StatusCode::UNAUTHORIZED,
            "invalid executor api key".to_string(),
        ));
    }
    let mut pending = state.file_read_pending.write().unwrap();
    let tx = pending.remove(&req.request_id).ok_or((
        StatusCode::NOT_FOUND,
        "request not found or expired".to_string(),
    ))?;
    drop(pending);
    let result = if let Some(e) = req.error {
        Err(e)
    } else if let Some(c) = req.content {
        Ok(c)
    } else {
        Err("missing content and error".to_string())
    };
    let _ = tx.send(result);
    Ok(StatusCode::NO_CONTENT)
}

/// Search repo for files by name. Requires JWT (controller). Relayer forwards to executor via WebSocket.
async fn files_search(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<FilesSearchQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let _ = extract_bearer_from_headers(&headers).and_then(|t| verify_bearer(&t, &state))?;
    if q.repo_path.is_empty() || q.file_name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "repo_path and file_name required".to_string(),
        ));
    }
    let request_id = Uuid::new_v4();
    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut pending = state.file_search_pending.write().unwrap();
        pending.insert(request_id, tx);
    }
    state.relay.broadcast(BroadcastMessage::FileSearchRequest(
        WsFileSearchRequestPayload {
            request_id,
            repo_path: q.repo_path.clone(),
            file_name: q.file_name.clone(),
        },
    ));
    let result = tokio::time::timeout(std::time::Duration::from_secs(120), rx).await;
    {
        let mut pending = state.file_search_pending.write().unwrap();
        pending.remove(&request_id);
    }
    match result {
        Ok(Ok(Ok(matches))) => Ok(Json(serde_json::json!({ "matches": matches }))),
        Ok(Ok(Err(e))) => Err((StatusCode::BAD_REQUEST, e)),
        Ok(Err(_)) => Err((
            StatusCode::GATEWAY_TIMEOUT,
            "executor did not respond in time".to_string(),
        )),
        Err(_) => Err((
            StatusCode::GATEWAY_TIMEOUT,
            "executor did not respond in time".to_string(),
        )),
    }
}

/// Executor responds with file search results. Requires EXECUTOR_API_KEY.
async fn files_search_response(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<FileSearchResponseRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    if token != state.config.executor_api_key {
        return Err((
            StatusCode::UNAUTHORIZED,
            "invalid executor api key".to_string(),
        ));
    }
    let mut pending = state.file_search_pending.write().unwrap();
    let tx = pending.remove(&req.request_id).ok_or((
        StatusCode::NOT_FOUND,
        "request not found or expired".to_string(),
    ))?;
    drop(pending);
    let result = if let Some(e) = req.error {
        Err(e)
    } else if let Some(m) = req.matches {
        Ok(m)
    } else {
        Err("missing matches and error".to_string())
    };
    let _ = tx.send(result);
    Ok(StatusCode::NO_CONTENT)
}

/// Sync repos from executor. Requires EXECUTOR_API_KEY. Replaces admin's repos with paths.
async fn repos_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SyncReposRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let token = extract_bearer_from_headers(&headers)?;
    if token != state.config.executor_api_key {
        return Err((
            StatusCode::UNAUTHORIZED,
            "invalid executor api key".to_string(),
        ));
    }
    let conn = state.db.0.lock().unwrap();
    let admin_id: String = conn
        .query_row("SELECT id FROM admin LIMIT 1", [], |row| row.get(0))
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "no admin".to_string()))?;
    let admin_id = Uuid::parse_str(&admin_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db::replace_repos(&conn, admin_id, &req.paths)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

// --- WebSocket ---

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Expect first message to be {"type":"auth","payload":{"token":"..."}}. Validate token,
/// send auth_ok or auth_fail, then subscribe to relay only if valid.
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    let token = match ws_rx.next().await {
        Some(Ok(Message::Text(t))) => {
            let envelope: Result<shared::WsEnvelope, _> = serde_json::from_str(&t);
            match envelope {
                Ok(e) if e.r#type == shared::ws_types::AUTH => e
                    .payload
                    .get("token")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                _ => None,
            }
        }
        _ => None,
    };

    let token = match token {
        Some(t) if !t.is_empty() => t,
        _ => {
            let _ = ws_tx
                .send(Message::Text(
                    serde_json::json!({
                        "type": shared::ws_types::AUTH_FAIL,
                        "payload": {"reason": "missing or invalid auth message"}
                    })
                    .to_string()
                    .into(),
                ))
                .await;
            return;
        }
    };

    // Validate: JWT (controller) or EXECUTOR_API_KEY (executor)
    let valid = if token == state.config.executor_api_key {
        let conn = state.db.0.lock().unwrap();
        conn.query_row("SELECT id FROM admin LIMIT 1", [], |row| {
            row.get::<_, String>(0)
        })
        .ok()
        .is_some()
    } else {
        crate::auth::validate_jwt(&token, &state.config.jwt_secret)
            .map(|o| o.is_some())
            .unwrap_or(false)
    };

    if !valid {
        let _ = ws_tx
            .send(Message::Text(
                serde_json::json!({
                    "type": shared::ws_types::AUTH_FAIL,
                    "payload": {"reason": "invalid token"}
                })
                .to_string()
                .into(),
            ))
            .await;
        return;
    }

    let _ = ws_tx
        .send(Message::Text(
            serde_json::json!({"type": shared::ws_types::AUTH_OK, "payload": {}})
                .to_string()
                .into(),
        ))
        .await;

    let mut rx = state.relay.subscribe();
    let mut ping_interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                recv = rx.recv() => {
                    let msg = match recv {
                        Ok(m) => m,
                        Err(_) => break,
                    };
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
                BroadcastMessage::FileReadRequest(p) => {
                    serde_json::to_string(&shared::WsEnvelope {
                        version: 1,
                        r#type: shared::ws_types::FILE_READ_REQUEST.to_string(),
                        payload: serde_json::to_value(p).unwrap(),
                        ts: Some(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()),
                    })
                }
                BroadcastMessage::FileSearchRequest(p) => {
                    serde_json::to_string(&shared::WsEnvelope {
                        version: 1,
                        r#type: shared::ws_types::FILE_SEARCH_REQUEST.to_string(),
                        payload: serde_json::to_value(p).unwrap(),
                        ts: Some(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()),
                    })
                }
            };
                    if let Ok(j) = json {
                        let _ = ws_tx.send(Message::Text(j.into())).await;
                    }
                }
                _ = ping_interval.tick() => {
                    let _ = ws_tx.send(Message::Ping(axum::body::Bytes::new())).await;
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{router, AppState};
    use crate::auth::{create_jwt, generate_api_key, generate_totp_secret, hash_api_key};
    use crate::config::Config;
    use crate::db;
    use crate::relay::RelayState;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::env;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn client_hash(password: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update("test-salt".as_bytes());
        hasher.update(b":dev-pm-agent:");
        hasher.update(password.as_bytes());
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }

    #[tokio::test]
    async fn commands_update_rejects_controller_jwt() {
        let executor_key = "test-executor-key-abc";
        let jwt_secret = "test-jwt-secret-xyz";
        let salt = "test-salt";

        let migrations_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("migrations")
            .canonicalize()
            .unwrap();
        env::set_var("MIGRATIONS_DIR", migrations_dir);

        let db_path = std::env::temp_dir().join(format!("relayer_test_{}.db", Uuid::new_v4()));
        let config = Config::for_test(db_path.clone(), jwt_secret, executor_key, salt);
        let config = Arc::new(config);

        let db = db::Db::open(&db_path).unwrap();
        db.run_migrations().unwrap();
        let db = Arc::new(db);

        let api_key = generate_api_key();
        let api_key_hash = hash_api_key(&api_key).unwrap();
        let totp_secret = generate_totp_secret().unwrap();
        let ch = client_hash("p");
        let password_hash = bcrypt::hash(format!("{}{}", salt, ch), bcrypt::DEFAULT_COST).unwrap();

        let conn = db.0.lock().unwrap();
        db::setup_admin(&conn, "admin1", &password_hash, &totp_secret, &api_key_hash).unwrap();
        let (device_id, admin_id, _) = db::validate_device(&conn, &api_key).unwrap().unwrap();
        let cmd_id =
            db::create_command(&conn, device_id, "test input", None, None, None, None, None)
                .unwrap();
        drop(conn);

        let controller_jwt =
            create_jwt(device_id, admin_id, "controller", &config.jwt_secret, 3600).unwrap();

        let state = AppState {
            db: db.clone(),
            relay: Arc::new(RelayState::new()),
            config: config.clone(),
            models: Arc::new(std::sync::RwLock::new(vec!["model1".to_string()])),
            file_read_pending: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            file_search_pending: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        };

        let app = router(state);

        let body = serde_json::json!({
            "status": "done",
            "output": "forged",
            "summary": "forged"
        });
        let req = Request::builder()
            .method("PATCH")
            .uri(format!("/api/commands/{}", cmd_id))
            .header("Authorization", format!("Bearer {}", controller_jwt))
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();

        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "controller JWT must not be allowed to update command status"
        );
    }

    #[tokio::test]
    async fn commands_update_accepts_executor_api_key() {
        let executor_key = "test-executor-key-def";
        let jwt_secret = "test-jwt-secret-uvw";
        let salt = "test-salt";

        let migrations_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("migrations")
            .canonicalize()
            .unwrap();
        env::set_var("MIGRATIONS_DIR", migrations_dir);

        let db_path = std::env::temp_dir().join(format!("relayer_test_{}.db", Uuid::new_v4()));
        let config = Config::for_test(db_path.clone(), jwt_secret, executor_key, salt);
        let config = Arc::new(config);

        let db = db::Db::open(&db_path).unwrap();
        db.run_migrations().unwrap();
        let db = Arc::new(db);

        let api_key = generate_api_key();
        let api_key_hash = hash_api_key(&api_key).unwrap();
        let totp_secret = generate_totp_secret().unwrap();
        let ch = client_hash("p");
        let password_hash = bcrypt::hash(format!("{}{}", salt, ch), bcrypt::DEFAULT_COST).unwrap();

        let conn = db.0.lock().unwrap();
        db::setup_admin(&conn, "admin1", &password_hash, &totp_secret, &api_key_hash).unwrap();
        let (device_id, _, _) = db::validate_device(&conn, &api_key).unwrap().unwrap();
        let cmd_id =
            db::create_command(&conn, device_id, "test input", None, None, None, None, None)
                .unwrap();
        drop(conn);

        let state = AppState {
            db: db.clone(),
            relay: Arc::new(RelayState::new()),
            config: config.clone(),
            models: Arc::new(std::sync::RwLock::new(vec!["model1".to_string()])),
            file_read_pending: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            file_search_pending: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        };

        let app = router(state);

        let body = serde_json::json!({
            "status": "done",
            "output": "ok",
            "summary": "ok"
        });
        let req = Request::builder()
            .method("PATCH")
            .uri(format!("/api/commands/{}", cmd_id))
            .header("Authorization", format!("Bearer {}", executor_key))
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();

        assert_eq!(
            response.status(),
            StatusCode::NO_CONTENT,
            "executor API key must be allowed to update command status"
        );
    }
}
