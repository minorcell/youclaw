export type WsKind = "request" | "response" | "event"

export interface WsErrorPayload {
  code: string
  message: string
}

export interface WsEnvelope<T = unknown> {
  id: string
  kind: WsKind
  name: string
  payload: T
  ok?: boolean
  error?: WsErrorPayload
}

export interface ProviderProfile {
  id: string
  provider_id: string
  model_name: string
  name: string
  base_url: string
  api_key: string
  model: string
  created_at: string
  updated_at: string
}

export interface ProviderModel {
  id: string
  provider_id: string
  name: string
  model: string
  created_at: string
  updated_at: string
}

export interface ProviderAccount {
  id: string
  name: string
  base_url: string
  api_key: string
  models: ProviderModel[]
  created_at: string
  updated_at: string
}

export interface ChatSession {
  id: string
  title: string
  provider_profile_id: string | null
  created_at: string
  updated_at: string
  last_run_at: string | null
}

export type ContentPart =
  | { Text: string }
  | { Reasoning: ReasoningPart }
  | { ToolCall: ToolCall }
  | { ToolResult: ToolResult }

export interface ReasoningPart {
  text: string
  provider_metadata?: Record<string, unknown>
}

export interface ChatMessage {
  id: string
  session_id: string
  role: "system" | "user" | "assistant" | "tool"
  parts_json: ContentPart[]
  run_id: string | null
  created_at: string
}

export interface ToolApproval {
  id: string
  session_id: string
  run_id: string
  call_id: string
  action: string
  path: string
  preview_json: {
    path?: string
    diff?: string
    old_excerpt?: string
    new_excerpt?: string
  }
  status: string
  created_at: string
  resolved_at: string | null
}

export interface ChatRun {
  id: string
  session_id: string
  status: string
  user_message: string
  output_text: string
  created_at: string
  finished_at: string | null
  error_message: string | null
}

export interface AgentActiveHoursConfig {
  start: string
  end: string
}

export interface AgentHeartbeatConfig {
  enabled: boolean
  every: string
  target: string
  active_hours?: AgentActiveHoursConfig | null
}

export interface AgentConfigPayload {
  max_steps: number
  max_input_tokens: number
  compact_ratio: number
  keep_recent: number
  language: string
  heartbeat: AgentHeartbeatConfig
}

export interface WorkspaceFileInfo {
  path: string
  size: number
  modified_at: string
}

export interface Usage {
  input_tokens: number
  input_no_cache_tokens: number
  input_cache_read_tokens: number
  input_cache_write_tokens: number
  output_tokens: number
  output_text_tokens: number
  reasoning_tokens: number
  total_tokens: number
  raw_usage?: Record<string, unknown> | null
}

export interface ToolCall {
  call_id: string
  tool_name: string
  args_json: Record<string, unknown>
}

export interface ToolResult {
  call_id: string
  output_json: Record<string, unknown>
  is_error: boolean
}

export interface AgentStep {
  step: number
  output_text: string
  reasoning_text: string
  reasoning_parts: ReasoningPart[]
  finish_reason: string
  usage: Usage
  tool_calls: ToolCall[]
  tool_results: ToolResult[]
}

export interface BootstrapPayload {
  provider_profiles: ProviderProfile[]
  provider_accounts: ProviderAccount[]
  sessions: ChatSession[]
  messages: ChatMessage[]
  approvals: ToolApproval[]
  runs: ChatRun[]
  last_opened_session_id: string | null
  agent_config: AgentConfigPayload
  workspace_files: WorkspaceFileInfo[]
}

export interface ProvidersChangedPayload {
  provider_profiles: ProviderProfile[]
  provider_accounts: ProviderAccount[]
}

export interface SessionsChangedPayload {
  sessions: ChatSession[]
  last_opened_session_id: string | null
}

export interface RunStartedPayload {
  session_id: string
  run: ChatRun
  user_message: ChatMessage
}

export interface TokenPayload {
  session_id: string
  run_id: string
  step: number
  text: string
}

export interface ReasoningStartedPayload {
  session_id: string
  run_id: string
  step: number
  block_id: string
  provider_metadata?: Record<string, unknown>
}

export interface ReasoningTokenPayload {
  session_id: string
  run_id: string
  step: number
  block_id: string
  text: string
  provider_metadata?: Record<string, unknown>
}

export interface ReasoningFinishedPayload {
  session_id: string
  run_id: string
  step: number
  block_id: string
  provider_metadata?: Record<string, unknown>
}

export interface StepStartedPayload {
  session_id: string
  run_id: string
  step: number
}

export interface StepFinishedPayload {
  session_id: string
  run_id: string
  step: AgentStep
}

export interface ToolRequestedPayload {
  session_id: string
  run_id: string
  step: number
  state: string
  tool_call: ToolCall
  approval: ToolApproval | null
}

export interface ToolFinishedPayload {
  session_id: string
  run_id: string
  step: number
  tool_call: ToolCall
  tool_result: ToolResult
  duration_ms: number
}

export interface RunFinishedPayload {
  session_id: string
  run: ChatRun
  new_messages: ChatMessage[]
  usage_total: Usage
}

export interface RunFailedPayload {
  session_id: string
  run_id: string
  error: string
}

export interface RunCancelledPayload {
  session_id: string
  run_id: string
}

export interface AgentMemoryCompactedPayload {
  session_id: string
  compacted_messages: number
  summary_preview: string
}

export interface AgentHeartbeatExecutedPayload {
  session_id: string
  status: string
  run_id?: string | null
  reason?: string | null
}

export type TimelineItem =
  | {
      id: string
      kind: "step"
      step: number
      status: "started" | "finished"
      outputText: string
      reasoningText: string
      usage?: Usage
    }
  | {
      id: string
      kind: "tool"
      step: number
      state: string
      toolCall: ToolCall
      toolResult?: ToolResult
      durationMs?: number
      approval?: ToolApproval | null
    }

export interface RunViewState {
  run: ChatRun
  sessionId: string
  timeline: TimelineItem[]
  liveStepsById: Record<string, Extract<TimelineItem, { kind: "step" }>>
  usageTotal?: Usage
  error?: string
}

export type UsageStatsRange = "24h" | "7d" | "30d" | "all"

export interface UsagePage {
  page: number
  page_size: number
  total: number
  has_more: boolean
}

export interface UsageSummaryPayload {
  range: UsageStatsRange
  total_requests: number
  input_tokens: number
  output_tokens: number
  reasoning_tokens: number
  total_tokens: number
  input_cache_read_tokens: number
  input_cache_write_tokens: number
}

export interface UsageSettingsPayload {
  detail_logging_enabled: boolean
}

export interface UsageLogItem {
  run_id: string
  session_id: string
  status: string
  user_message: string
  provider_id: string | null
  provider_name: string | null
  model_id: string | null
  model_name: string | null
  model: string | null
  started_at: string
  finished_at: string | null
  duration_ms: number | null
  detail_logged: boolean
  input_tokens: number
  output_tokens: number
  reasoning_tokens: number
  total_tokens: number
  input_cache_read_tokens: number
  input_cache_write_tokens: number
}

export interface UsageLogsPayload {
  page: UsagePage
  items: UsageLogItem[]
}

export interface UsageProviderStatsItem {
  provider_id: string | null
  provider_name: string | null
  request_count: number
  completed_count: number
  failed_count: number
  cancelled_count: number
  input_tokens: number
  output_tokens: number
  total_tokens: number
  input_cache_read_tokens: number
  input_cache_write_tokens: number
}

export interface UsageProviderStatsPayload {
  page: UsagePage
  items: UsageProviderStatsItem[]
}

export interface UsageModelStatsItem {
  model_id: string | null
  model_name: string | null
  model: string | null
  provider_id: string | null
  provider_name: string | null
  request_count: number
  completed_count: number
  failed_count: number
  cancelled_count: number
  input_tokens: number
  output_tokens: number
  total_tokens: number
  input_cache_read_tokens: number
  input_cache_write_tokens: number
  avg_duration_ms: number | null
}

export interface UsageModelStatsPayload {
  page: UsagePage
  items: UsageModelStatsItem[]
}

export interface UsageToolStatsItem {
  tool_name: string
  tool_action: string | null
  call_count: number
  success_count: number
  error_count: number
  avg_duration_ms: number | null
}

export interface UsageToolStatsPayload {
  page: UsagePage
  items: UsageToolStatsItem[]
}

export interface UsageToolLogItem {
  id: string
  run_id: string
  session_id: string
  tool_name: string
  tool_action: string | null
  status: string
  duration_ms: number | null
  is_error: boolean
  created_at: string
}

export interface UsageLogDetailPayload {
  run_id: string
  tools: UsageToolLogItem[]
}
