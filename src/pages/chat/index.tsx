import { useEffect, useMemo, useRef, useState } from 'react'
import { Navigate, useParams } from 'react-router-dom'
import { useShallow } from 'zustand/react/shallow'

import { ChatComposer } from '@/pages/chat/components/chat-composer'
import { MessageThread } from '@/pages/chat/components/message-thread'
import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'
import { getAppClient } from '@/lib/app-client'
import { partsToOutputText } from '@/lib/parts'
import { flattenProviderProfiles } from '@/lib/provider-profiles'
import type { ChatMessage, ProviderProfile, TimelineItem } from '@/lib/types'
import { useAppStore } from '@/store/app-store'

const EMPTY_MESSAGES: ChatMessage[] = []
const WHITESPACE_REGEX = /\s+/g

function normalizeWhitespace(text: string): string {
  return text.replace(WHITESPACE_REGEX, ' ').trim()
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
  const activeRun = useAppStore((state) => {
    if (!sessionId) return null
    const activeRunId = state.activeRunIdBySession[sessionId]
    if (!activeRunId) return null
    return state.runsById[activeRunId] ?? null
  })
  const providers = useMemo(() => flattenProviderProfiles(providerAccounts), [providerAccounts])

  const [input, setInput] = useState('')
  const [userScrolledUp, setUserScrolledUp] = useState(false)

  const scrollContainerRef = useRef<HTMLDivElement>(null)
  const lastMessageCountRef = useRef(0)
  const lastRunStepsTextRef = useRef('')

  const activeSession = useMemo(
    () => sessions.find((session) => session.id === sessionId) ?? null,
    [sessions, sessionId],
  )

  const activeProvider = useMemo<ProviderProfile | null>(() => {
    if (!activeSession?.provider_profile_id) return null
    return providers.find((provider) => provider.id === activeSession.provider_profile_id) ?? null
  }, [activeSession, providers])

  const runSteps = useMemo(() => {
    if (!activeRun) return []
    const completedSteps = activeRun.timeline
      .filter((item): item is Extract<TimelineItem, { kind: 'step' }> => item.kind === 'step')
      .map((item) => ({
        step: item.step,
        status: item.status,
        outputText: item.outputText,
        reasoningText: item.reasoningText,
      }))
    const liveSteps = Object.values(activeRun.liveStepsById).map((item) => ({
      step: item.step,
      status: item.status,
      outputText: item.outputText,
      reasoningText: item.reasoningText,
    }))
    return [...completedSteps, ...liveSteps].sort((left, right) => left.step - right.step)
  }, [activeRun])

  const activeRunId = activeRun?.run.id ?? null
  const hasRunSteps = runSteps.length > 0
  const normalizedStepText = useMemo(() => {
    if (!hasRunSteps) return ''
    return normalizeWhitespace(runSteps[runSteps.length - 1].outputText)
  }, [hasRunSteps, runSteps])

  const normalizedMessageTextById = useMemo(() => {
    const normalizedById = new Map<string, string>()
    for (const message of messages) {
      normalizedById.set(message.id, normalizeWhitespace(partsToOutputText(message.parts_json)))
    }
    return normalizedById
  }, [messages])

  const renderMessages = useMemo(() => {
    if (!activeRunId || !hasRunSteps) return messages

    return messages.filter((message) => {
      if (message.role !== 'assistant' || message.run_id !== activeRunId) {
        return true
      }

      if (!normalizedStepText) {
        return false
      }

      return normalizedMessageTextById.get(message.id) !== normalizedStepText
    })
  }, [activeRunId, hasRunSteps, messages, normalizedMessageTextById, normalizedStepText])

  const pendingApprovals = useMemo(() => {
    if (!sessionId) return []
    return Object.values(approvalsById)
      .filter((approval) => approval.session_id === sessionId && approval.status === 'pending')
      .sort((left, right) => right.created_at.localeCompare(left.created_at))
  }, [approvalsById, sessionId])

  useEffect(() => {
    if (sessionId) {
      setActiveSession(sessionId)
    }
  }, [sessionId, setActiveSession])

  // 检测用户是否手动上滑
  useEffect(() => {
    const container = scrollContainerRef.current
    if (!container) return

    const handleScroll = () => {
      const currScrollTop = container.scrollTop
      const currScrollHeight = container.scrollHeight
      const currClientHeight = container.clientHeight
      const currIsNearBottom = currScrollHeight - currScrollTop - currClientHeight < 100
      if (!currIsNearBottom) {
        setUserScrolledUp(true)
      } else {
        setUserScrolledUp(false)
      }
    }

    container.addEventListener('scroll', handleScroll)
    return () => container.removeEventListener('scroll', handleScroll)
  }, [])

  // 当消息变化时，如果用户没有上滑，自动滚动到底部
  const currentMessageCount = messages.length
  useEffect(() => {
    const container = scrollContainerRef.current
    if (!container) return

    if (!userScrolledUp && currentMessageCount > lastMessageCountRef.current) {
      container.scrollTo({ top: container.scrollHeight })
    }
    lastMessageCountRef.current = currentMessageCount
  }, [currentMessageCount, userScrolledUp])

  // 当 runSteps 变化时（流式输出），如果用户没有上滑，自动滚动到底部
  const currentRunStepsText = useMemo(
    () => runSteps.map((step) => `${step.reasoningText}\n${step.outputText}`).join(''),
    [runSteps],
  )
  useEffect(() => {
    const container = scrollContainerRef.current
    if (!container) return

    if (!userScrolledUp && currentRunStepsText !== lastRunStepsTextRef.current) {
      requestAnimationFrame(() => {
        container.scrollTo({ top: container.scrollHeight, behavior: 'smooth' })
      })
    }
    lastRunStepsTextRef.current = currentRunStepsText
  }, [currentRunStepsText, userScrolledUp])

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
    clearError()
    await getAppClient().request('chat.send', {
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

  return (
    <div className='flex h-full min-h-0 flex-col bg-background/70'>
      <div className='relative flex-1 min-h-0'>
        <div
          ref={scrollContainerRef}
          className='h-full select-none overflow-y-auto px-6 pb-72 pt-8 md:px-[9%]'
        >
          <div className='select-text'>
            <MessageThread
              error={activeRun?.error}
              messages={renderMessages}
              providerLabel={
                activeProvider
                  ? `${activeProvider.name} / ${activeProvider.model_name || activeProvider.model}`
                  : 'BgtClaw Agent'
              }
              runSteps={runSteps}
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
              onInputChange={setInput}
              onSend={() => void handleSend()}
              providers={providers}
              selectedProviderId={activeSession.provider_profile_id}
            />
          </div>
        </div>
      </div>
    </div>
  )
}
