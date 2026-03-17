import type {
  ReasoningTokenPayload,
  TokenPayload,
  ToolApproval,
  ToolFinishedPayload,
  ToolRequestedPayload,
  TurnCancelledPayload,
  TurnFailedPayload,
  TurnFinishedPayload,
  TurnStartedPayload,
  StepFinishedPayload,
  StepStartedPayload,
  WsEnvelope,
} from '@/lib/types'
import type { AppStoreData } from '@/store/app-state'
import type { TimelineItem } from '@/store/types'

import { getOrCreateTurnView, mergeUniqueMessages, upsertTimelineItem } from './utils'

export function reduceChatEnvelope(
  state: AppStoreData,
  envelope: WsEnvelope,
): Partial<AppStoreData> | null {
  switch (envelope.name) {
    case 'chat.turn.started': {
      const payload = envelope.payload as TurnStartedPayload
      const messages = mergeUniqueMessages(state.messagesBySession[payload.session_id] ?? [], [
        payload.user_message,
      ])
      const current = getOrCreateTurnView(state, payload.turn)
      return {
        messagesBySession: {
          ...state.messagesBySession,
          [payload.session_id]: messages,
        },
        turnsById: {
          ...state.turnsById,
          [payload.turn.id]: {
            ...current,
            turn: payload.turn,
            sessionId: payload.session_id,
            timeline: [],
            liveStepsById: {},
            error: undefined,
          },
        },
        activeTurnIdBySession: {
          ...state.activeTurnIdBySession,
          [payload.session_id]: payload.turn.id,
        },
      }
    }
    case 'chat.step.token': {
      const payload = envelope.payload as TokenPayload
      return reduceStreamingStepUpdate(state, payload.turn_id, payload.step, (existingStep) =>
        existingStep
          ? {
              ...existingStep,
              outputText: `${existingStep.outputText}${payload.text}`,
            }
          : {
              id: `step-${payload.step}`,
              kind: 'step',
              step: payload.step,
              status: 'started',
              outputText: payload.text,
              reasoningText: '',
            },
      )
    }
    case 'chat.step.reasoning.token': {
      const payload = envelope.payload as ReasoningTokenPayload
      return reduceStreamingStepUpdate(state, payload.turn_id, payload.step, (existingStep) =>
        existingStep
          ? {
              ...existingStep,
              reasoningText: `${existingStep.reasoningText}${payload.text}`,
            }
          : {
              id: `step-${payload.step}`,
              kind: 'step',
              step: payload.step,
              status: 'started',
              outputText: '',
              reasoningText: payload.text,
            },
      )
    }
    case 'chat.step.started': {
      const payload = envelope.payload as StepStartedPayload
      const current = state.turnsById[payload.turn_id]
      if (!current) {
        return null
      }
      const stepId = `step-${payload.step}`
      const existingStep = current.liveStepsById[stepId]
      return {
        turnsById: {
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
        },
      }
    }
    case 'chat.step.finished': {
      const payload = envelope.payload as StepFinishedPayload
      const current = state.turnsById[payload.turn_id]
      if (!current) {
        return null
      }
      const stepId = `step-${payload.step.step}`
      const liveStepsById = { ...current.liveStepsById }
      delete liveStepsById[stepId]
      return {
        turnsById: {
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
        },
      }
    }
    case 'chat.step.tool.requested': {
      const payload = envelope.payload as ToolRequestedPayload
      const current = state.turnsById[payload.turn_id]
      if (!current) {
        return null
      }
      return {
        approvalsById: mergeApprovalMap(state.approvalsById, payload.approval),
        turnsById: {
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
        },
      }
    }
    case 'chat.step.tool.finished': {
      const payload = envelope.payload as ToolFinishedPayload
      const current = state.turnsById[payload.turn_id]
      if (!current) {
        return null
      }
      return {
        turnsById: {
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
        },
      }
    }
    case 'chat.turn.finished': {
      const payload = envelope.payload as TurnFinishedPayload
      const current = getOrCreateTurnView(state, payload.turn)
      const messages = mergeUniqueMessages(
        state.messagesBySession[payload.session_id] ?? [],
        payload.new_messages,
      )
      return {
        messagesBySession: {
          ...state.messagesBySession,
          [payload.session_id]: messages,
        },
        turnsById: {
          ...state.turnsById,
          [payload.turn.id]: {
            ...current,
            turn: payload.turn,
            liveStepsById: {},
            usageTotal: payload.usage_total,
            error: undefined,
          },
        },
      }
    }
    case 'chat.turn.failed': {
      const payload = envelope.payload as TurnFailedPayload
      const current = state.turnsById[payload.turn_id]
      if (!current) {
        return { lastError: payload.error }
      }
      return {
        turnsById: {
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
        },
        lastError: payload.error,
      }
    }
    case 'chat.turn.cancelled': {
      const payload = envelope.payload as TurnCancelledPayload
      const current = state.turnsById[payload.turn_id]
      if (!current) {
        return null
      }
      return {
        turnsById: {
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
        },
      }
    }
    default:
      return null
  }
}

function reduceStreamingStepUpdate(
  state: AppStoreData,
  turnId: string,
  step: number,
  update: (
    existingStep: Extract<TimelineItem, { kind: 'step' }> | undefined,
  ) => Extract<TimelineItem, { kind: 'step' }>,
): Partial<AppStoreData> | null {
  const current = state.turnsById[turnId]
  if (!current) {
    return null
  }
  const stepId = `step-${step}`
  const existingStep = current.liveStepsById[stepId]
  return {
    turnsById: {
      ...state.turnsById,
      [turnId]: {
        ...current,
        liveStepsById: {
          ...current.liveStepsById,
          [stepId]: update(existingStep),
        },
      },
    },
  }
}

function mergeApprovalMap(
  approvalsById: Record<string, ToolApproval>,
  approval: ToolApproval | null,
): Record<string, ToolApproval> {
  if (!approval) {
    return approvalsById
  }
  return {
    ...approvalsById,
    [approval.id]: approval,
  }
}
