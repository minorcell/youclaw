import { create } from 'zustand'

import type {
  BootstrapPayload,
  ChatMessage,
  ChatTurn,
  ChatSession,
  ProviderAccount,
  ProvidersChangedPayload,
  ReasoningTokenPayload,
  TurnCancelledPayload,
  TurnFailedPayload,
  TurnFinishedPayload,
  TurnStartedPayload,
  TurnViewState,
  SessionsChangedPayload,
  StepFinishedPayload,
  StepStartedPayload,
  TimelineItem,
  TokenPayload,
  ToolApproval,
  ToolFinishedPayload,
  ToolRequestedPayload,
  WsEnvelope,
} from '@/lib/types'

export type WsStatus = 'idle' | 'connecting' | 'open' | 'closed' | 'error'

interface AppStoreState {
  initialized: boolean
  wsStatus: WsStatus
  endpoint: string | null
  providerAccounts: ProviderAccount[]
  sessions: ChatSession[]
  messagesBySession: Record<string, ChatMessage[]>
  approvalsById: Record<string, ToolApproval>
  turnsById: Record<string, TurnViewState>
  activeTurnIdBySession: Record<string, string>
  activeSessionId: string | null
  lastOpenedSessionId: string | null
  lastError: string | null
  setWsStatus: (status: WsStatus) => void
  setEndpoint: (endpoint: string) => void
  setActiveSession: (sessionId: string | null) => void
  applyEnvelope: (envelope: WsEnvelope) => void
  clearError: () => void
}

function groupMessages(messages: ChatMessage[]): Record<string, ChatMessage[]> {
  const seenBySession: Record<string, Set<string>> = {}
  return messages.reduce<Record<string, ChatMessage[]>>((acc, message) => {
    acc[message.session_id] ??= []
    seenBySession[message.session_id] ??= new Set<string>()
    if (seenBySession[message.session_id].has(message.id)) {
      return acc
    }
    seenBySession[message.session_id].add(message.id)
    acc[message.session_id].push(message)
    return acc
  }, {})
}

function mergeUniqueMessages(messages: ChatMessage[], incoming: ChatMessage[]): ChatMessage[] {
  if (incoming.length === 0) {
    return messages
  }
  const seen = new Set(messages.map((item) => item.id))
  const next = [...messages]
  let changed = false
  for (const message of incoming) {
    if (seen.has(message.id)) {
      continue
    }
    seen.add(message.id)
    next.push(message)
    changed = true
  }
  return changed ? next : messages
}

function upsertTimelineItem(items: TimelineItem[], nextItem: TimelineItem): TimelineItem[] {
  const index = items.findIndex((item) => item.id === nextItem.id)
  if (index === -1) {
    return [...items, nextItem]
  }

  const clone = [...items]
  clone[index] = nextItem
  return clone
}

function getOrCreateTurnView(state: AppStoreState, turn: ChatTurn): TurnViewState {
  return (
    state.turnsById[turn.id] ?? {
      turn,
      sessionId: turn.session_id,
      timeline: [],
      liveStepsById: {},
      usageTotal: undefined,
      error: turn.error_message ?? undefined,
    }
  )
}

function buildActiveTurnIdBySession(turns: ChatTurn[]): Record<string, string> {
  const activeTurnBySession: Record<string, ChatTurn> = {}
  for (const turn of turns) {
    const current = activeTurnBySession[turn.session_id]
    if (!current || turn.created_at.localeCompare(current.created_at) > 0) {
      activeTurnBySession[turn.session_id] = turn
    }
  }
  return Object.fromEntries(
    Object.entries(activeTurnBySession).map(([sessionId, turn]) => [sessionId, turn.id]),
  )
}

export const useAppStore = create<AppStoreState>((set) => ({
  initialized: false,
  wsStatus: 'idle',
  endpoint: null,
  providerAccounts: [],
  sessions: [],
  messagesBySession: {},
  approvalsById: {},
  turnsById: {},
  activeTurnIdBySession: {},
  activeSessionId: null,
  lastOpenedSessionId: null,
  lastError: null,
  setWsStatus: (wsStatus) => set({ wsStatus }),
  setEndpoint: (endpoint) => set({ endpoint }),
  setActiveSession: (activeSessionId) => set({ activeSessionId }),
  clearError: () => set({ lastError: null }),
  applyEnvelope: (envelope) => {
    set((state) => {
      const next: Partial<AppStoreState> = {}

      if (envelope.kind === 'response' && envelope.ok === false && envelope.error) {
        next.lastError = envelope.error.message
        return next as AppStoreState
      }

      switch (envelope.name) {
        case 'bootstrap.get': {
          const payload = envelope.payload as BootstrapPayload
          if (!payload || !('provider_accounts' in payload)) {
            return state
          }
          next.initialized = true
          next.providerAccounts = payload.provider_accounts ?? []
          next.sessions = payload.sessions
          next.messagesBySession = groupMessages(payload.messages)
          next.approvalsById = Object.fromEntries(
            payload.approvals.map((approval) => [approval.id, approval]),
          )
          next.turnsById = Object.fromEntries(
            payload.turns.map((turn) => [
              turn.id,
              {
                turn,
                sessionId: turn.session_id,
                timeline: [],
                liveStepsById: {},
                error: turn.error_message ?? undefined,
              },
            ]),
          )
          next.activeTurnIdBySession = buildActiveTurnIdBySession(payload.turns)
          next.lastOpenedSessionId = payload.last_opened_session_id
          next.activeSessionId =
            state.activeSessionId ??
            payload.last_opened_session_id ??
            payload.sessions[0]?.id ??
            null
          next.lastError = null
          return next as AppStoreState
        }
        case 'providers.changed': {
          const payload = envelope.payload as ProvidersChangedPayload
          next.providerAccounts = payload.provider_accounts ?? []
          return next as AppStoreState
        }
        case 'providers.list': {
          const payload = envelope.payload as ProvidersChangedPayload
          next.providerAccounts = payload.provider_accounts ?? []
          return next as AppStoreState
        }
        case 'sessions.changed':
        case 'sessions.list': {
          const payload = envelope.payload as SessionsChangedPayload
          next.sessions = payload.sessions
          next.lastOpenedSessionId = payload.last_opened_session_id
          return next as AppStoreState
        }
        case 'tool_approvals.resolve': {
          const approval = envelope.payload as ToolApproval
          next.approvalsById = {
            ...state.approvalsById,
            [approval.id]: approval,
          }
          next.lastError = null
          return next as AppStoreState
        }
        case 'chat.turn.started': {
          const payload = envelope.payload as TurnStartedPayload
          const messages = mergeUniqueMessages(state.messagesBySession[payload.session_id] ?? [], [
            payload.user_message,
          ])
          const current = getOrCreateTurnView(state, payload.turn)
          next.messagesBySession = {
            ...state.messagesBySession,
            [payload.session_id]: messages,
          }
          next.turnsById = {
            ...state.turnsById,
            [payload.turn.id]: {
              ...current,
              turn: payload.turn,
              sessionId: payload.session_id,
              timeline: [],
              liveStepsById: {},
              error: undefined,
            },
          }
          next.activeTurnIdBySession = {
            ...state.activeTurnIdBySession,
            [payload.session_id]: payload.turn.id,
          }
          return next as AppStoreState
        }
        case 'chat.step.token': {
          const payload = envelope.payload as TokenPayload
          const current = state.turnsById[payload.turn_id]
          if (!current) return state
          const stepId = `step-${payload.step}`
          const existingStep = current.liveStepsById[stepId]
          const nextStep: Extract<TimelineItem, { kind: 'step' }> = existingStep
            ? {
                ...existingStep,
                outputText: `${existingStep.outputText}${payload.text}`,
              }
            : {
                id: stepId,
                kind: 'step',
                step: payload.step,
                status: 'started',
                outputText: payload.text,
                reasoningText: '',
              }
          next.turnsById = {
            ...state.turnsById,
            [payload.turn_id]: {
              ...current,
              liveStepsById: {
                ...current.liveStepsById,
                [stepId]: nextStep,
              },
            },
          }
          return next as AppStoreState
        }
        case 'chat.step.reasoning.token': {
          const payload = envelope.payload as ReasoningTokenPayload
          const current = state.turnsById[payload.turn_id]
          if (!current) return state
          const stepId = `step-${payload.step}`
          const existingStep = current.liveStepsById[stepId]
          const nextStep: Extract<TimelineItem, { kind: 'step' }> = existingStep
            ? {
                ...existingStep,
                reasoningText: `${existingStep.reasoningText}${payload.text}`,
              }
            : {
                id: stepId,
                kind: 'step',
                step: payload.step,
                status: 'started',
                outputText: '',
                reasoningText: payload.text,
              }
          next.turnsById = {
            ...state.turnsById,
            [payload.turn_id]: {
              ...current,
              liveStepsById: {
                ...current.liveStepsById,
                [stepId]: nextStep,
              },
            },
          }
          return next as AppStoreState
        }
        case 'chat.step.started': {
          const payload = envelope.payload as StepStartedPayload
          const current = state.turnsById[payload.turn_id]
          if (!current) return state
          const stepId = `step-${payload.step}`
          const existingStep = current.liveStepsById[stepId]
          next.turnsById = {
            ...state.turnsById,
            [payload.turn_id]: {
              ...current,
              liveStepsById: {
                ...current.liveStepsById,
                [stepId]: {
                  id: stepId,
                  kind: 'step',
                  step: payload.step,
                  status: 'started',
                  outputText: existingStep?.outputText ?? '',
                  reasoningText: existingStep?.reasoningText ?? '',
                },
              },
            },
          }
          return next as AppStoreState
        }
        case 'chat.step.finished': {
          const payload = envelope.payload as StepFinishedPayload
          const current = state.turnsById[payload.turn_id]
          if (!current) return state
          const stepId = `step-${payload.step.step}`
          const liveStepsById = { ...current.liveStepsById }
          delete liveStepsById[stepId]
          next.turnsById = {
            ...state.turnsById,
            [payload.turn_id]: {
              ...current,
              timeline: upsertTimelineItem(current.timeline, {
                id: stepId,
                kind: 'step',
                step: payload.step.step,
                status: 'finished',
                outputText: payload.step.output_text,
                reasoningText: payload.step.reasoning_text ?? '',
                usage: payload.step.usage,
              }),
              liveStepsById,
            },
          }
          return next as AppStoreState
        }
        case 'chat.step.tool.requested': {
          const payload = envelope.payload as ToolRequestedPayload
          const current = state.turnsById[payload.turn_id]
          if (!current) return state
          const approvalsById = { ...state.approvalsById }
          if (payload.approval) {
            approvalsById[payload.approval.id] = payload.approval
          }
          next.approvalsById = approvalsById
          next.turnsById = {
            ...state.turnsById,
            [payload.turn_id]: {
              ...current,
              timeline: upsertTimelineItem(current.timeline, {
                id: `tool-${payload.tool_call.call_id}`,
                kind: 'tool',
                step: payload.step,
                state: payload.state,
                toolCall: payload.tool_call,
                approval: payload.approval,
              }),
            },
          }
          return next as AppStoreState
        }
        case 'chat.step.tool.finished': {
          const payload = envelope.payload as ToolFinishedPayload
          const current = state.turnsById[payload.turn_id]
          if (!current) return state
          next.turnsById = {
            ...state.turnsById,
            [payload.turn_id]: {
              ...current,
              timeline: upsertTimelineItem(current.timeline, {
                id: `tool-${payload.tool_call.call_id}`,
                kind: 'tool',
                step: payload.step,
                state: 'finished',
                toolCall: payload.tool_call,
                toolResult: payload.tool_result,
                durationMs: payload.duration_ms,
              }),
            },
          }
          return next as AppStoreState
        }
        case 'chat.turn.finished': {
          const payload = envelope.payload as TurnFinishedPayload
          const current = getOrCreateTurnView(state, payload.turn)
          const messages = mergeUniqueMessages(
            state.messagesBySession[payload.session_id] ?? [],
            payload.new_messages,
          )
          next.messagesBySession = {
            ...state.messagesBySession,
            [payload.session_id]: messages,
          }
          next.turnsById = {
            ...state.turnsById,
            [payload.turn.id]: {
              ...current,
              turn: payload.turn,
              liveStepsById: {},
              usageTotal: payload.usage_total,
              error: undefined,
            },
          }
          return next as AppStoreState
        }
        case 'chat.turn.failed': {
          const payload = envelope.payload as TurnFailedPayload
          const current = state.turnsById[payload.turn_id]
          if (!current) {
            next.lastError = payload.error
            return next as AppStoreState
          }
          next.turnsById = {
            ...state.turnsById,
            [payload.turn_id]: {
              ...current,
              turn: {
                ...current.turn,
                status: 'failed',
                error_message: payload.error,
              },
              liveStepsById: {},
              error: payload.error,
            },
          }
          next.lastError = payload.error
          return next as AppStoreState
        }
        case 'chat.turn.cancelled': {
          const payload = envelope.payload as TurnCancelledPayload
          const current = state.turnsById[payload.turn_id]
          if (!current) return state
          next.turnsById = {
            ...state.turnsById,
            [payload.turn_id]: {
              ...current,
              turn: {
                ...current.turn,
                status: 'cancelled',
                error_message: 'Turn cancelled',
              },
              liveStepsById: {},
              error: 'Turn cancelled',
            },
          }
          return next as AppStoreState
        }
        default:
          return state
      }
    })
  },
}))
