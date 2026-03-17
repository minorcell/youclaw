import type { ToolApproval, ToolResult } from '@/lib/types'

export interface ToolRenderUnit {
  callId: string
  toolName: string
  argsJson: Record<string, unknown>
  result?: ToolResult
  durationMs?: number
  isLive: boolean
  approval?: ToolApproval | null
}

export interface StepRenderUnit {
  step: number
  isLive: boolean
  outputText: string
  reasoningText: string
  tools: ToolRenderUnit[]
}

export interface TurnRenderUnit {
  turnId: string
  userText: string
  steps: StepRenderUnit[]
  status: string
  isActive: boolean
  error?: string
}
