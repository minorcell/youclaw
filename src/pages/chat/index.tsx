import { useEffect, useMemo, useRef, useState } from 'react'
import { Navigate, useParams } from 'react-router-dom'
import { useShallow } from 'zustand/react/shallow'

import { ChatComposer } from '@/pages/chat/components/chat-composer'
import { MessageThread } from '@/pages/chat/components/message-thread'
import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'
import { getAppClient } from '@/lib/app-client'
import { partsToOutputText, partsToReasoningDisplay } from '@/lib/parts'
import { flattenProviderProfiles } from '@/lib/provider-profiles'
import type {
  AgentStep,
  ChatMessage,
  ProviderProfile,
  StepRenderUnit,
  TimelineItem,
  ToolApproval,
  ToolCall,
  ToolRenderUnit,
  TurnRenderUnit,
  TurnStepsListPayload,
  TurnViewState,
} from '@/lib/types'
import { useAppStore } from '@/store/app-store'

const EMPTY_MESSAGES: ChatMessage[] = []
const EMPTY_TURNS: TurnViewState[] = []

// ---- Turn render unit builders ----

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
    if (!byStep.has(step)) byStep.set(step, { stepItem: undefined, toolItems: [] })
    return byStep.get(step)!
  }

  for (const item of timeline) {
    const accum = getOrCreate(item.step)
    if (item.kind === 'step') {
      accum.stepItem = item
    } else if (item.kind === 'tool') {
      accum.toolItems.push(item)
    }
  }

  for (const liveStep of Object.values(liveStepsById)) {
    getOrCreate(liveStep.step).stepItem = liveStep
  }

  return Array.from(byStep.entries())
    .sort(([a], [b]) => a - b)
    .flatMap(([, accum]) => {
      if (!accum.stepItem) return []
      const { stepItem, toolItems } = accum
      const tools: ToolRenderUnit[] = toolItems.map((toolItem) => ({
        callId: toolItem.toolCall.call_id,
        toolName: toolItem.toolCall.tool_name,
        argsJson: toolItem.toolCall.args_json,
        result: toolItem.toolResult,
        durationMs: toolItem.durationMs,
        isLive: toolItem.state !== 'finished',
        approval:
          toolItem.approval ??
          Object.values(approvalsById).find((a) => a.call_id === toolItem.toolCall.call_id) ??
          null,
      }))
      return [
        {
          step: stepItem.step,
          isLive: stepItem.status === 'started',
          outputText: stepItem.outputText,
          reasoningText: stepItem.reasoningText,
          tools,
        } satisfies StepRenderUnit,
      ]
    })
}

function buildStepsFromAgentSteps(agentSteps: AgentStep[]): StepRenderUnit[] {
  return agentSteps.map((agentStep) => {
    const resultByCallId = new Map(agentStep.tool_results.map((r) => [r.call_id, r]))
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
    .filter((m) => m.turn_id === turnId && m.role !== 'system')
    .sort((a, b) => a.created_at.localeCompare(b.created_at))

  // Collect all tool results from this turn (from tool-role messages)
  const allToolResults = new Map<string, { call_id: string; output_json: Record<string, unknown>; is_error: boolean }>()
  for (const msg of turnMessages) {
    if (msg.role !== 'tool' && msg.role !== 'assistant') continue
    for (const part of msg.parts_json) {
      if ('ToolResult' in part) {
        allToolResults.set(part.ToolResult.call_id, part.ToolResult)
      }
    }
  }

  const steps: StepRenderUnit[] = []
  let stepIndex = 0

  for (const msg of turnMessages) {
    if (msg.role !== 'assistant') continue

    const outputText = partsToOutputText(msg.parts_json)
    const reasoningText = partsToReasoningDisplay(msg.parts_json)
    const toolCalls: ToolCall[] = msg.parts_json.flatMap((p) =>
      'ToolCall' in p ? [p.ToolCall] : [],
    )

    const tools: ToolRenderUnit[] = toolCalls.map((call) => ({
      callId: call.call_id,
      toolName: call.tool_name,
      argsJson: call.args_json,
      result: allToolResults.get(call.call_id),
      isLive: false,
      approval:
        Object.values(approvalsById).find((a) => a.call_id === call.call_id) ?? null,
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
    stepIndex++
  }

  return steps
}

export function ChatPage() {
  const params = useParams<{ sessionId: string }>()
  const sessionId = params.sessionId ?? null

  const { providerAccounts, sessions, approvalsById, setActiveSession, clearError } = useAppStore(
    useShallow((state) => ({
      providerAccounts: state.providerAccounts,
      sessions: state.sessions,
      approvalsById: state.approvalsById,
      setActiveSession: state.setActiveSession,
      clearError: state.clearError,
    })),
  )

  const messages = useAppStore((state) =>
    sessionId ? (state.messagesBySession[sessionId] ?? EMPTY_MESSAGES) : EMPTY_MESSAGES,
  )

  const activeTurnId = useAppStore((state) =>
    sessionId ? (state.activeTurnIdBySession[sessionId] ?? null) : null,
  )
  const activeTurnStatus = useAppStore((state) => {
    if (!activeTurnId) return null
    return state.turnsById[activeTurnId]?.turn.status ?? null
  })

  const turnsForSession = useAppStore(
    useShallow((state) => {
      if (!sessionId) return EMPTY_TURNS
      return Object.values(state.turnsById)
        .filter((tv) => tv.sessionId === sessionId)
        .sort((a, b) => a.turn.created_at.localeCompare(b.turn.created_at))
    }),
  )

  const providers = useMemo(() => flattenProviderProfiles(providerAccounts), [providerAccounts])

  const [input, setInput] = useState('')
  const [persistedStepsByTurnId, setPersistedStepsByTurnId] = useState<Record<string, AgentStep[]>>(
    {},
  )

  const scrollContainerRef = useRef<HTMLDivElement>(null)
  const scrolledUpRef = useRef(false)
  const lastTurnCountRef = useRef(0)
  const lastStepTextRef = useRef('')

  const activeSession = useMemo(
    () => sessions.find((session) => session.id === sessionId) ?? null,
    [sessions, sessionId],
  )

  const activeProvider = useMemo<ProviderProfile | null>(() => {
    if (!activeSession?.provider_profile_id) return null
    return providers.find((provider) => provider.id === activeSession.provider_profile_id) ?? null
  }, [activeSession, providers])

  // Load persisted steps for the active turn (provides fallback before WS events arrive)
  useEffect(() => {
    if (!activeTurnId) return
    let disposed = false
    ;(async () => {
      try {
        const payload = await getAppClient().request<TurnStepsListPayload>('chat.turn.steps.list', {
          turn_id: activeTurnId,
        })
        if (disposed) return
        setPersistedStepsByTurnId((current) => ({
          ...current,
          [activeTurnId]: payload.steps.sort((left, right) => left.step - right.step),
        }))
      } catch {
        if (!disposed) {
          setPersistedStepsByTurnId((current) => ({
            ...current,
            [activeTurnId]: current[activeTurnId] ?? [],
          }))
        }
      }
    })()
    return () => {
      disposed = true
    }
  }, [activeTurnId])

  // Build unified turn render units from all data sources
  const turnRenderUnits = useMemo<TurnRenderUnit[]>(() => {
    return turnsForSession.map((turnViewState) => {
      const { turn } = turnViewState
      const isActive = turn.id === activeTurnId

      const hasTimelineData =
        turnViewState.timeline.length > 0 ||
        Object.keys(turnViewState.liveStepsById).length > 0

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
  }, [turnsForSession, activeTurnId, persistedStepsByTurnId, messages, approvalsById])

  const pendingApprovals = useMemo(() => {
    if (!sessionId) return []
    return Object.values(approvalsById)
      .filter((approval) => approval.session_id === sessionId && approval.status === 'pending')
      .sort((left, right) => right.created_at.localeCompare(left.created_at))
  }, [approvalsById, sessionId])

  const isTurnRunning = activeTurnStatus === 'running'

  useEffect(() => {
    if (sessionId) {
      setActiveSession(sessionId)
    }
  }, [sessionId, setActiveSession])

  // Detect user scroll intent: wheel up → pause auto-scroll; near bottom → resume
  useEffect(() => {
    const container = scrollContainerRef.current
    if (!container) return
    const handleWheel = (e: WheelEvent) => {
      if (e.deltaY < 0) scrolledUpRef.current = true
    }
    const handleScroll = () => {
      const { scrollTop, scrollHeight, clientHeight } = container
      if (scrollHeight - scrollTop - clientHeight < 60) {
        scrolledUpRef.current = false
      }
    }
    container.addEventListener('wheel', handleWheel, { passive: true })
    container.addEventListener('scroll', handleScroll, { passive: true })
    return () => {
      container.removeEventListener('wheel', handleWheel)
      container.removeEventListener('scroll', handleScroll)
    }
  }, [])

  // Auto-scroll when new turns arrive
  const turnCount = turnRenderUnits.length
  useEffect(() => {
    const container = scrollContainerRef.current
    if (!container || scrolledUpRef.current) return
    if (turnCount > lastTurnCountRef.current) {
      container.scrollTo({ top: container.scrollHeight })
    }
    lastTurnCountRef.current = turnCount
  }, [turnCount])

  // Auto-scroll during streaming
  const lastStepText = useMemo(() => {
    const lastTurn = turnRenderUnits[turnRenderUnits.length - 1]
    if (!lastTurn) return ''
    const lastStep = lastTurn.steps[lastTurn.steps.length - 1]
    if (!lastStep) return ''
    return `${lastStep.reasoningText}${lastStep.outputText}`
  }, [turnRenderUnits])

  useEffect(() => {
    if (lastStepText === lastStepTextRef.current) return
    lastStepTextRef.current = lastStepText
    const container = scrollContainerRef.current
    if (!container || scrolledUpRef.current) return
    requestAnimationFrame(() => {
      if (!scrolledUpRef.current) {
        container.scrollTo({ top: container.scrollHeight })
      }
    })
  }, [lastStepText])

  if (providers.length === 0) {
    return <Navigate replace to='/welcome/provider' />
  }

  if (!activeSession) {
    return <Navigate replace to='/' />
  }

  const activeSessionId = activeSession.id

  async function handleSend() {
    const text = input.trim()
    if (!text) return
    setInput('')
    scrolledUpRef.current = false
    clearError()
    await getAppClient().request('chat.turn.start', {
      session_id: activeSessionId,
      text,
    })
  }

  async function handleBindProvider(providerProfileId: string | null) {
    if (!providerProfileId) return
    await getAppClient().request('sessions.bind_provider', {
      session_id: activeSessionId,
      provider_profile_id: providerProfileId,
    })
  }

  async function handleResolveApproval(approvalId: string, approved: boolean) {
    await getAppClient().request('tool_approvals.resolve', {
      approval_id: approvalId,
      approved,
    })
  }

  async function handleCancelTurn() {
    if (!activeTurnId || !isTurnRunning) return
    await getAppClient().request('chat.turn.cancel', {
      turn_id: activeTurnId,
    })
  }

  return (
    <div className='flex h-full min-h-0 flex-col bg-background/70'>
      <div className='relative flex-1 min-h-0'>
        <div
          ref={scrollContainerRef}
          className='h-full select-none overflow-y-auto px-6 pb-72 pt-8 md:px-[9%]'
        >
          <div className='select-text'>
            <MessageThread
              providerLabel={
                activeProvider
                  ? `${activeProvider.name} / ${activeProvider.model_name || activeProvider.model}`
                  : 'YouClaw Agent'
              }
              turns={turnRenderUnits}
            />
          </div>

          {pendingApprovals.length > 0 ? (
            <div className='mt-6 space-y-3 select-text'>
              {pendingApprovals.map((approval) => (
                <Card
                  className='max-w-[76ch] rounded-2xl border-border/70 bg-card/80 px-4 py-3 shadow-none'
                  key={approval.id}
                >
                  <div className='flex items-center justify-between gap-2'>
                    <p className='truncate text-sm font-medium text-foreground'>{approval.path}</p>
                    <Badge>{approval.action}</Badge>
                  </div>
                  <pre className='mt-2 max-h-40 overflow-auto rounded-xl bg-muted/70 p-3 text-[11px] leading-5 text-foreground/80'>
                    {approval.preview_json.diff ?? 'No diff preview'}
                  </pre>
                  <div className='mt-3 flex gap-2'>
                    <button
                      className='rounded-full border border-border/70 bg-background px-3 py-1.5 text-xs hover:bg-muted'
                      onClick={() => void handleResolveApproval(approval.id, true)}
                      type='button'
                    >
                      允许
                    </button>
                    <button
                      className='rounded-full border border-border/70 bg-background px-3 py-1.5 text-xs hover:bg-muted'
                      onClick={() => void handleResolveApproval(approval.id, false)}
                      type='button'
                    >
                      拒绝
                    </button>
                  </div>
                </Card>
              ))}
            </div>
          ) : null}
        </div>

        <div className='pointer-events-none absolute inset-x-0 bottom-3 flex justify-center px-4'>
          <div className='pointer-events-auto w-full max-w-210 select-none'>
            <ChatComposer
              input={input}
              onBindProvider={(id) => void handleBindProvider(id)}
              onCancelTurn={() => void handleCancelTurn()}
              onInputChange={setInput}
              onSend={() => void handleSend()}
              providers={providers}
              selectedProviderId={activeSession.provider_profile_id}
              isTurnRunning={isTurnRunning}
            />
          </div>
        </div>
      </div>
    </div>
  )
}
