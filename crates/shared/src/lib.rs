//! Shared types and models for the Dev PM Agent monorepo.

mod models;

// Explicit re-exports (avoids rust-analyzer issues with `pub use models::*`)
pub use models::ws_types;
pub use models::{
    AddRepoRequest, BootstrapDeviceResponse, ChatHistoryEntry, CommandResponse, CommandStatus,
    CreateCommandRequest, DeviceRole, FileReadResponseRequest, FileSearchMatch,
    FileSearchResponseRequest, LoginRequest, LoginResponse, RefreshRequest, RefreshResponse,
    RegisterDeviceRequest, RegisterDeviceResponse, RepoResponse, ReserveCodeRequest,
    ReserveCodeResponse, SetupRequest, SetupResponse, SyncModelsRequest, SyncReposRequest,
    UpdateCommandRequest, VerifyBootstrapRequest, VerifyBootstrapResponse, WsAuthPayload,
    WsCommandAckPayload, WsCommandNewPayload, WsCommandResultPayload, WsCommandUpdatePayload,
    WsEnvelope, WsFileReadRequestPayload, WsFileSearchRequestPayload,
};
