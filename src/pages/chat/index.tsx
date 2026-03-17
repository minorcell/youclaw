import { useEffect, useMemo, useState } from 'react'
import { Navigate, useParams } from 'react-router-dom'
import { useShallow } from 'zustand/react/shallow'

import { ChatComposer } from '@/pages/chat/components/chat-composer'
import { MessageThread } from '@/pages/chat/components/message-thread'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useToastContext } from '@/contexts/toast-context'
import { getAppClient } from '@/lib/app-client'
import { flattenProviderProfiles } from '@/lib/provider-profiles'
import type { ChatMessage, ProviderProfile, SessionApprovalMode } from '@/lib/types'
import { buildTurnRenderUnits } from '@/pages/chat/adapters/build-turn-render-units'
import { useChatScroll } from '@/pages/chat/hooks/use-chat-scroll'
import { usePersistedTurnSteps } from '@/pages/chat/hooks/use-persisted-turn-steps'
import { useAppStore } from '@/store/app-store'
import type { TurnViewState } from '@/store/types'

import { ToolApprovalCard } from './components/tool-approval-card'

const EMPTY_MESSAGES: ChatMessage[] = []
const EMPTY_TURNS: TurnViewState[] = []

export function ChatPage() {
  const params = useParams<{ sessionId: string }>()
  const sessionId = params.sessionId ?? null
  const { error: toastError } = useToastContext()

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
  const [approvalModeBusy, setApprovalModeBusy] = useState(false)
  const persistedStepsByTurnId = usePersistedTurnSteps(activeTurnId)

  const activeSession = useMemo(
    () => sessions.find((session) => session.id === sessionId) ?? null,
    [sessions, sessionId],
  )

  const activeProvider = useMemo<ProviderProfile | null>(() => {
    if (!activeSession?.provider_profile_id) return null
    return providers.find((provider) => provider.id === activeSession.provider_profile_id) ?? null
  }, [activeSession, providers])

  const turnRenderUnits = useMemo(
    () =>
      buildTurnRenderUnits({
        turnsForSession,
        activeTurnId,
        persistedStepsByTurnId,
        messages,
        approvalsById,
      }),
    [turnsForSession, activeTurnId, persistedStepsByTurnId, messages, approvalsById],
  )

  const { scrollContainerRef, resetAutoScroll } = useChatScroll(turnRenderUnits)

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
    resetAutoScroll()
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

  async function handleApprovalModeChange(approvalMode: SessionApprovalMode) {
    if (!activeSession || approvalModeBusy || activeSession.approval_mode === approvalMode) {
      return
    }
    setApprovalModeBusy(true)
    try {
      await getAppClient().request('sessions.update_approval_mode', {
        session_id: activeSessionId,
        approval_mode: approvalMode,
      })
    } catch (error) {
      toastError(error instanceof Error ? error.message : String(error))
    } finally {
      setApprovalModeBusy(false)
    }
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
        <ScrollArea
          className='h-full select-none'
          hideScrollbar
          viewportClassName='no-scrollbar'
          viewportRef={scrollContainerRef}
        >
          <div className='px-6 pb-72 pt-8 md:px-[9%]'>
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
                  <ToolApprovalCard
                    key={approval.id}
                    approval={approval}
                    onResolveApproval={(approvalId, approved) =>
                      void handleResolveApproval(approvalId, approved)
                    }
                  />
                ))}
              </div>
            ) : null}
          </div>
        </ScrollArea>

        <div className='pointer-events-none absolute inset-x-0 bottom-3 flex justify-center px-4'>
          <div className='pointer-events-auto w-full max-w-210 select-none'>
            <ChatComposer
              approvalMode={activeSession.approval_mode}
              approvalModeBusy={approvalModeBusy}
              input={input}
              onApprovalModeChange={(mode) => void handleApprovalModeChange(mode)}
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
