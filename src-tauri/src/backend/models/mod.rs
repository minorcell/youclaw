mod common;
mod config;
pub mod domain;
pub mod events;
mod provider;
pub mod requests;
pub mod responses;
mod session;
mod usage;
mod ws;

// Transitional flat exports while callers migrate to `models::domain`.
#[allow(unused_imports)]
// Transitional flat exports while callers migrate to `models::events`.
#[allow(unused_imports)]
pub use events::{
    AgentMemoryCompactedPayload, ConnectionReadyPayload, ReasoningFinishedPayload,
    ReasoningStartedPayload, ReasoningTokenPayload, StepFinishedPayload, StepStartedPayload,
    TokenPayload, ToolFinishedPayload, ToolRequestedPayload, TurnCancelledPayload,
    TurnFailedPayload, TurnFinishedPayload, TurnStartedPayload,
};
// Transitional flat exports while callers migrate to `models::responses`.
#[allow(unused_imports)]
pub use responses::{
    ArchivedSessionsPayload, BootstrapPayload, MemoryGetPayload, MemoryReindexPayload,
    MemorySearchHit, MemorySearchPayload, ProvidersChangedPayload, SessionsChangedPayload,
    TurnStepsListPayload, WorkspaceFileInfo, WorkspaceFileReadPayload, WorkspaceFileWritePayload,
    WorkspaceFilesPayload,
};
// Transitional flat exports while callers migrate to `models::domain`.
#[allow(unused_imports)]
pub use domain::{
    flatten_provider_profiles, message_from_record, new_chat_session, new_chat_turn,
    new_provider_account, new_provider_model, new_tool_approval, new_user_chat_message,
    now_timestamp, record_from_message, title_from_first_prompt, update_provider_account,
    update_provider_model, AgentConfigPayload, ChatMessage, ChatSession, ChatTurn, MessageRole,
    ProviderAccount, ProviderModel, ProviderProfile, SessionApprovalMode, StoredProviders,
    ToolApproval, TurnStatus,
};
// Transitional flat exports while callers migrate to `models::requests`.
#[allow(unused_imports)]
pub use requests::{
    AgentConfigUpdateRequest, BindSessionProviderRequest, ChatTurnCancelRequest,
    ChatTurnStartRequest, CreateProviderModelRequest, CreateProviderRequest, CreateSessionRequest,
    DeleteProviderModelRequest, DeleteSessionRequest, MemoryGetRequest, MemorySearchRequest,
    PurgeSessionRequest, RenameSessionRequest, RestoreSessionRequest, TestProviderModelRequest,
    ToolApprovalResolveRequest, TurnStepsListRequest, UpdateProviderModelRequest,
    UpdateProviderRequest, UpdateSessionApprovalModeRequest, UsageLogDetailRequest,
    UsageLogsListRequest, UsageStatsListRequest, UsageSummaryRequest, WorkspaceFileReadRequest,
    WorkspaceFileWriteRequest,
};
pub use usage::{
    UsageLogDetailPayload, UsageLogItem, UsageLogsPayload, UsageModelStatsItem,
    UsageModelStatsPayload, UsagePage, UsageProviderStatsItem, UsageProviderStatsPayload,
    UsageSummaryPayload, UsageToolLogItem, UsageToolStatsItem, UsageToolStatsPayload,
    USAGE_RANGE_24H, USAGE_RANGE_30D, USAGE_RANGE_7D, USAGE_RANGE_ALL,
};
pub use ws::{WsEnvelope, WsKind};
