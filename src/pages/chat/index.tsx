import { useEffect, useMemo, useState } from "react"
import { Navigate, useParams } from "react-router-dom"

import { ChatComposer } from "@/pages/chat/components/chat-composer"
import { MessageThread } from "@/pages/chat/components/message-thread"
import { Badge } from "@/components/ui/badge"
import { Card } from "@/components/ui/card"
import { getAppClient } from "@/lib/app-client"
import { partsToText } from "@/lib/parts"
import type { ProviderProfile, RunViewState, TimelineItem } from "@/lib/types"
import { useAppStore } from "@/store/app-store"

export function ChatPage() {
  const params = useParams<{ sessionId: string }>()
  const sessionId = params.sessionId ?? null

  const providers = useAppStore((state) => state.providers)
  const sessions = useAppStore((state) => state.sessions)
  const messagesBySession = useAppStore((state) => state.messagesBySession)
  const runsById = useAppStore((state) => state.runsById)
  const approvalsById = useAppStore((state) => state.approvalsById)
  const wsStatus = useAppStore((state) => state.wsStatus)
  const setActiveSession = useAppStore((state) => state.setActiveSession)
  const clearError = useAppStore((state) => state.clearError)

  const [input, setInput] = useState("")

  const activeSession = useMemo(
    () => sessions.find((session) => session.id === sessionId) ?? null,
    [sessions, sessionId],
  )

  const activeProvider = useMemo<ProviderProfile | null>(() => {
    if (!activeSession?.provider_profile_id) return null
    return (
      providers.find(
        (provider) => provider.id === activeSession.provider_profile_id,
      ) ?? null
    )
  }, [activeSession, providers])

  const messages = sessionId ? (messagesBySession[sessionId] ?? []) : []
  const activeRun = useMemo<RunViewState | null>(() => {
    if (!sessionId) return null
    return (
      Object.values(runsById)
        .filter((run) => run.sessionId === sessionId)
        .sort((left, right) =>
          right.run.created_at.localeCompare(left.run.created_at),
        )[0] ?? null
    )
  }, [runsById, sessionId])

  const runSteps = useMemo(() => {
    if (!activeRun) return []
    return activeRun.timeline
      .filter(
        (item): item is Extract<TimelineItem, { kind: "step" }> =>
          item.kind === "step",
      )
      .map((item) => ({
        step: item.step,
        status: item.status,
        outputText: item.outputText,
      }))
      .sort((left, right) => left.step - right.step)
  }, [activeRun])

  const renderMessages = useMemo(() => {
    if (!activeRun || runSteps.length === 0) return messages

    const lastStep = runSteps[runSteps.length - 1]
    const normalizedStepText = lastStep.outputText.replace(/\s+/g, " ").trim()

    return messages.filter((message) => {
      if (message.role !== "assistant" || message.run_id !== activeRun.run.id) {
        return true
      }

      const normalizedMessageText = partsToText(message.parts_json)
        .replace(/\s+/g, " ")
        .trim()

      if (!normalizedStepText) {
        return false
      }

      return normalizedMessageText !== normalizedStepText
    })
  }, [activeRun, messages, runSteps])

  const pendingApprovals = useMemo(() => {
    if (!sessionId) return []
    return Object.values(approvalsById)
      .filter(
        (approval) =>
          approval.session_id === sessionId && approval.status === "pending",
      )
      .sort((left, right) => right.created_at.localeCompare(left.created_at))
  }, [approvalsById, sessionId])

  useEffect(() => {
    if (sessionId) {
      setActiveSession(sessionId)
    }
  }, [sessionId, setActiveSession])

  if (providers.length === 0) {
    return <Navigate replace to="/welcome/provider" />
  }

  if (!activeSession) {
    return <Navigate replace to="/" />
  }

  const activeSessionId = activeSession.id

  async function handleSend() {
    const text = input.trim()
    if (!text) return
    setInput("")
    clearError()
    await getAppClient().request("chat.send", {
      session_id: activeSessionId,
      text,
    })
  }

  async function handleBindProvider(providerProfileId: string | null) {
    if (!providerProfileId) return
    await getAppClient().request("sessions.bind_provider", {
      session_id: activeSessionId,
      provider_profile_id: providerProfileId,
    })
  }

  async function handleResolveApproval(approvalId: string, approved: boolean) {
    await getAppClient().request("tool_approvals.resolve", {
      approval_id: approvalId,
      approved,
    })
  }

  return (
    <div className="flex h-full min-h-0 flex-col bg-background/70">
      <div className="relative flex-1 min-h-0">
        <div className="h-full overflow-y-auto px-6 pb-72 pt-8 md:px-[9%]">
          <div className="mb-7 flex items-center justify-center gap-2">
            <Badge className="rounded-full border-border bg-muted/60 px-3 py-1 text-xs text-muted-foreground">
              今日
            </Badge>
            <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground/80">
              {wsStatus}
            </span>
          </div>

          <MessageThread
            error={activeRun?.error}
            messages={renderMessages}
            providerLabel={
              activeProvider
                ? `${activeProvider.name} / ${activeProvider.model}`
                : "Baogongtou Agent"
            }
            runSteps={runSteps}
          />

          {pendingApprovals.length > 0 ? (
            <div className="mt-6 space-y-3">
              {pendingApprovals.map((approval) => (
                <Card
                  className="max-w-[76ch] rounded-2xl border-border/70 bg-card/80 px-4 py-3 shadow-none"
                  key={approval.id}
                >
                  <div className="flex items-center justify-between gap-2">
                    <p className="truncate text-sm font-medium text-foreground">
                      {approval.path}
                    </p>
                    <Badge>{approval.action}</Badge>
                  </div>
                  <pre className="mt-2 max-h-40 overflow-auto rounded-xl bg-muted/70 p-3 text-[11px] leading-5 text-foreground/80">
                    {approval.preview_json.diff ?? "No diff preview"}
                  </pre>
                  <div className="mt-3 flex gap-2">
                    <button
                      className="rounded-full border border-border/70 bg-background px-3 py-1.5 text-xs hover:bg-muted"
                      onClick={() =>
                        void handleResolveApproval(approval.id, true)
                      }
                      type="button"
                    >
                      允许
                    </button>
                    <button
                      className="rounded-full border border-border/70 bg-background px-3 py-1.5 text-xs hover:bg-muted"
                      onClick={() =>
                        void handleResolveApproval(approval.id, false)
                      }
                      type="button"
                    >
                      拒绝
                    </button>
                  </div>
                </Card>
              ))}
            </div>
          ) : null}
        </div>

        <div className="pointer-events-none absolute inset-x-0 bottom-6 flex justify-center px-4">
          <div className="pointer-events-auto w-full max-w-[840px]">
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
