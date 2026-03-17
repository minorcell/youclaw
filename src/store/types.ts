import type { ChatTurn, ToolApproval, ToolCall, ToolResult, Usage } from '@/lib/types'

export type TimelineItem =
  | {
      id: string
      kind: 'step'
      step: number
      status: 'started' | 'finished'
      outputText: string
      reasoningText: string
      usage?: Usage
    }
  | {
      id: string
      kind: 'tool'
      step: number
      state: string
      toolCall: ToolCall
      toolResult?: ToolResult
      durationMs?: number
      approval?: ToolApproval | null
    }

export interface TurnViewState {
  turn: ChatTurn
  sessionId: string
  timeline: TimelineItem[]
  liveStepsById: Record<string, Extract<TimelineItem, { kind: 'step' }>>
  usageTotal?: Usage
  error?: string
}
