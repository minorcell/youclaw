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
  | { ToolCall: ToolCall }
  | { ToolResult: ToolResult }

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

export interface Usage {
  input_tokens: number
  output_tokens: number
  total_tokens: number
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
  messages: ChatMessage[]
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

export type TimelineItem =
  | {
      id: string
      kind: "step"
      step: number
      status: "started" | "finished"
      outputText: string
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
  usageTotal?: Usage
  error?: string
}
