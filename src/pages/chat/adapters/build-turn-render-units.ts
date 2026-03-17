import { partsToOutputText, partsToReasoningDisplay } from '@/lib/parts'
import type {
  AgentStep,
  ChatMessage,
  ToolApproval,
  ToolCall,
} from '@/lib/types'
import type { TurnViewState, TimelineItem } from '@/store/types'

import type { StepRenderUnit, ToolRenderUnit, TurnRenderUnit } from '../types'

interface BuildTurnRenderUnitsInput {
  turnsForSession: TurnViewState[]
  activeTurnId: string | null
  persistedStepsByTurnId: Record<string, AgentStep[]>
  messages: ChatMessage[]
  approvalsById: Record<string, ToolApproval>
}

export function buildTurnRenderUnits({
  turnsForSession,
  activeTurnId,
  persistedStepsByTurnId,
  messages,
  approvalsById,
}: BuildTurnRenderUnitsInput): TurnRenderUnit[] {
  return turnsForSession.map((turnViewState) => {
    const { turn } = turnViewState
    const isActive = turn.id === activeTurnId
    const hasTimelineData =
      turnViewState.timeline.length > 0 || Object.keys(turnViewState.liveStepsById).length > 0

    let steps: StepRenderUnit[]
    if (hasTimelineData) {
      steps = buildStepsFromTimeline(
        turnViewState.timeline,
        turnViewState.liveStepsById,
        approvalsById,
      )
    } else if (persistedStepsByTurnId[turn.id]) {
      steps = buildStepsFromAgentSteps(persistedStepsByTurnId[turn.id])
    } else {
      steps = buildStepsFromMessages(messages, turn.id, approvalsById)
    }

    return {
      turnId: turn.id,
      userText: turn.user_message,
      steps,
      status: turn.status,
      isActive,
      error: turnViewState.error,
    }
  })
}

function buildStepsFromTimeline(
  timeline: TimelineItem[],
  liveStepsById: Record<string, Extract<TimelineItem, { kind: 'step' }>>,
  approvalsById: Record<string, ToolApproval>,
): StepRenderUnit[] {
  type StepAccum = {
    stepItem: Extract<TimelineItem, { kind: 'step' }> | undefined
    toolItems: Array<Extract<TimelineItem, { kind: 'tool' }>>
  }

  const byStep = new Map<number, StepAccum>()

  const getOrCreate = (step: number): StepAccum => {
    if (!byStep.has(step)) {
      byStep.set(step, { stepItem: undefined, toolItems: [] })
    }
    return byStep.get(step)!
  }

  for (const item of timeline) {
    const accum = getOrCreate(item.step)
    if (item.kind === 'step') {
      accum.stepItem = item
    } else {
      accum.toolItems.push(item)
    }
  }

  for (const liveStep of Object.values(liveStepsById)) {
    getOrCreate(liveStep.step).stepItem = liveStep
  }

  return Array.from(byStep.entries())
    .sort(([left], [right]) => left - right)
    .flatMap(([, accum]) => {
      if (!accum.stepItem) {
        return []
      }
      const tools: ToolRenderUnit[] = accum.toolItems.map((toolItem) => ({
        callId: toolItem.toolCall.call_id,
        toolName: toolItem.toolCall.tool_name,
        argsJson: toolItem.toolCall.args_json,
        result: toolItem.toolResult,
        durationMs: toolItem.durationMs,
        isLive: toolItem.state !== 'finished',
        approval:
          toolItem.approval ??
          Object.values(approvalsById).find((approval) => approval.call_id === toolItem.toolCall.call_id) ??
          null,
      }))
      return [
        {
          step: accum.stepItem.step,
          isLive: accum.stepItem.status === 'started',
          outputText: accum.stepItem.outputText,
          reasoningText: accum.stepItem.reasoningText,
          tools,
        } satisfies StepRenderUnit,
      ]
    })
}

function buildStepsFromAgentSteps(agentSteps: AgentStep[]): StepRenderUnit[] {
  return agentSteps.map((agentStep) => {
    const resultByCallId = new Map(agentStep.tool_results.map((result) => [result.call_id, result]))
    const tools: ToolRenderUnit[] = agentStep.tool_calls.map((call) => ({
      callId: call.call_id,
      toolName: call.tool_name,
      argsJson: call.args_json,
      result: resultByCallId.get(call.call_id),
      isLive: false,
      approval: null,
    }))
    return {
      step: agentStep.step,
      isLive: false,
      outputText: agentStep.output_text,
      reasoningText: agentStep.reasoning_text ?? '',
      tools,
    }
  })
}

function buildStepsFromMessages(
  messages: ChatMessage[],
  turnId: string,
  approvalsById: Record<string, ToolApproval>,
): StepRenderUnit[] {
  const turnMessages = messages
    .filter((message) => message.turn_id === turnId && message.role !== 'system')
    .sort((left, right) => left.created_at.localeCompare(right.created_at))

  const allToolResults = new Map<
    string,
    { call_id: string; output_json: Record<string, unknown>; is_error: boolean }
  >()

  for (const message of turnMessages) {
    if (message.role !== 'tool' && message.role !== 'assistant') {
      continue
    }
    for (const part of message.parts_json) {
      if ('ToolResult' in part) {
        allToolResults.set(part.ToolResult.call_id, part.ToolResult)
      }
    }
  }

  const steps: StepRenderUnit[] = []
  let stepIndex = 0

  for (const message of turnMessages) {
    if (message.role !== 'assistant') {
      continue
    }

    const outputText = partsToOutputText(message.parts_json)
    const reasoningText = partsToReasoningDisplay(message.parts_json)
    const toolCalls: ToolCall[] = message.parts_json.flatMap((part) =>
      'ToolCall' in part ? [part.ToolCall] : [],
    )

    const tools: ToolRenderUnit[] = toolCalls.map((call) => ({
      callId: call.call_id,
      toolName: call.tool_name,
      argsJson: call.args_json,
      result: allToolResults.get(call.call_id),
      isLive: false,
      approval: Object.values(approvalsById).find((approval) => approval.call_id === call.call_id) ?? null,
    }))

    if (outputText || reasoningText || tools.length > 0) {
      steps.push({
        step: stepIndex,
        isLive: false,
        outputText,
        reasoningText,
        tools,
      })
    }
    stepIndex += 1
  }

  return steps
}
