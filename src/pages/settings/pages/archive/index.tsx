import {
  ArchiveRestore,
  Clock3,
  Loader2,
  RefreshCw,
  Trash2,
  Waypoints,
} from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { useToastContext } from '@/contexts/toast-context'
import { getAppClient } from '@/lib/app-client'
import { flattenProviderProfiles } from '@/lib/provider-profiles'
import type { ArchivedSessionsPayload, ChatSession } from '@/lib/types'
import { useAppStore } from '@/store/app-store'

function errorMessageFromUnknown(error: unknown): string {
  if (typeof error === 'string') {
    return error
  }
  if (error instanceof Error) {
    return error.message
  }
  if (
    typeof error === 'object' &&
    error !== null &&
    'message' in error &&
    typeof error.message === 'string'
  ) {
    return error.message
  }
  return '操作失败，请稍后重试。'
}

function formatDateTime(value: string | null | undefined): string {
  if (!value) return '-'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) {
    return value
  }
  return date.toLocaleString('zh-CN', { hour12: false })
}

export function ArchiveSettingsPage() {
  const { success: toastSuccess, error: toastError } = useToastContext()
  const initialized = useAppStore((state) => state.initialized)
  const providerAccounts = useAppStore((state) => state.providerAccounts)
  const sessions = useAppStore((state) => state.sessions)
  const [archivedSessions, setArchivedSessions] = useState<ChatSession[]>([])
  const [loading, setLoading] = useState(false)
  const [busyActionId, setBusyActionId] = useState<string | null>(null)
  const [confirmPurgeSessionId, setConfirmPurgeSessionId] = useState<string | null>(null)
  const requestIdRef = useRef(0)

  const providers = useMemo(() => flattenProviderProfiles(providerAccounts), [providerAccounts])
  const providerLabels = useMemo(
    () =>
      new Map(
        providers.map((provider) => [
          provider.id,
          `${provider.name} / ${provider.model_name || provider.model}`,
        ]),
      ),
    [providers],
  )
  const sessionsSignature = useMemo(
    () => sessions.map((session) => `${session.id}:${session.updated_at}`).join('|'),
    [sessions],
  )

  async function loadArchivedSessions(silent = false) {
    const requestId = requestIdRef.current + 1
    requestIdRef.current = requestId
    if (!silent) {
      setLoading(true)
    }
    try {
      const payload = await getAppClient().request<ArchivedSessionsPayload>(
        'sessions.archived.list',
        {},
        { skipDispatch: true },
      )
      if (requestId !== requestIdRef.current) {
        return
      }
      setArchivedSessions(payload.sessions ?? [])
    } catch (error) {
      if (requestId !== requestIdRef.current) {
        return
      }
      toastError(errorMessageFromUnknown(error))
    } finally {
      if (requestId === requestIdRef.current) {
        setLoading(false)
      }
    }
  }

  useEffect(() => {
    if (!initialized) {
      return
    }
    void loadArchivedSessions(requestIdRef.current > 0)
  }, [initialized, sessionsSignature])

  async function handleRestore(sessionId: string) {
    const busyId = `restore:${sessionId}`
    if (!initialized || busyActionId) {
      return
    }
    setBusyActionId(busyId)
    try {
      await getAppClient().request(
        'sessions.restore',
        { session_id: sessionId },
        { skipDispatch: true },
      )
      setArchivedSessions((current) => current.filter((session) => session.id !== sessionId))
      setConfirmPurgeSessionId((current) => (current === sessionId ? null : current))
      toastSuccess('已恢复到最近会话。')
    } catch (error) {
      toastError(errorMessageFromUnknown(error))
    } finally {
      setBusyActionId(null)
    }
  }

  async function handlePurge(sessionId: string) {
    if (!initialized || busyActionId) {
      return
    }
    if (confirmPurgeSessionId !== sessionId) {
      setConfirmPurgeSessionId(sessionId)
      return
    }

    const busyId = `purge:${sessionId}`
    setBusyActionId(busyId)
    try {
      await getAppClient().request(
        'sessions.purge',
        { session_id: sessionId },
        { skipDispatch: true },
      )
      setArchivedSessions((current) => current.filter((session) => session.id !== sessionId))
      setConfirmPurgeSessionId(null)
      toastSuccess('已彻底删除。')
    } catch (error) {
      toastError(errorMessageFromUnknown(error))
    } finally {
      setBusyActionId(null)
    }
  }

  return (
    <Card className='bg-card/80 py-0 shadow-none'>
      <CardHeader className='py-4'>
        <div className='flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between'>
          <div>
            <CardTitle>归档记录</CardTitle>
            <CardDescription>这里的会话不会出现在左侧最近会话里。</CardDescription>
          </div>
          <div className='flex items-center gap-2'>
            <Badge className='bg-background text-foreground'>{archivedSessions.length} 条</Badge>
            <Button
              disabled={!initialized || loading || busyActionId !== null}
              onClick={() => {
                setConfirmPurgeSessionId(null)
                void loadArchivedSessions()
              }}
              size='sm'
              type='button'
              variant='outline'
            >
              {loading ? (
                <Loader2 className='mr-1 h-3.5 w-3.5 animate-spin' />
              ) : (
                <RefreshCw className='mr-1 h-3.5 w-3.5' />
              )}
              刷新
            </Button>
          </div>
        </div>
      </CardHeader>

      <CardContent className='space-y-3 py-2'>
        {loading && archivedSessions.length === 0 ? (
          <div className='rounded-2xl bg-background/80 px-4 py-6 text-sm text-muted-foreground'>
            正在载入归档记录...
          </div>
        ) : null}

        {!loading && archivedSessions.length === 0 ? (
          <div className='rounded-2xl bg-background/80 px-4 py-8 text-center'>
            <Waypoints className='mx-auto h-8 w-8 text-muted-foreground/70' />
            <p className='mt-3 text-sm font-medium text-foreground'>还没有归档记录</p>
            <p className='mt-1 text-xs text-muted-foreground'>归档后的会话会出现在这里。</p>
          </div>
        ) : null}

        {archivedSessions.map((session) => {
          const providerLabel = session.provider_profile_id
            ? providerLabels.get(session.provider_profile_id) ?? '已删除的模型'
            : '未绑定模型'
          const isRestoreBusy = busyActionId === `restore:${session.id}`
          const isPurgeBusy = busyActionId === `purge:${session.id}`
          const isBusy = busyActionId !== null
          const isConfirmingPurge = confirmPurgeSessionId === session.id

          return (
            <div className='rounded-2xl bg-background/85 p-4' key={session.id}>
              <div className='flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between'>
                <div className='min-w-0 flex-1'>
                  <div className='flex flex-wrap items-center gap-2'>
                    <p className='truncate text-sm font-medium text-foreground'>{session.title}</p>
                    <Badge className='bg-muted text-muted-foreground'>已归档</Badge>
                  </div>

                  <div className='mt-3 flex flex-wrap items-center gap-2 text-xs text-muted-foreground'>
                    <Badge className='bg-card text-foreground'>{providerLabel}</Badge>
                    <span className='inline-flex items-center gap-1'>
                      <Clock3 className='h-3.5 w-3.5' />
                      归档于 {formatDateTime(session.archived_at)}
                    </span>
                  </div>

                  {isConfirmingPurge ? (
                    <p className='mt-3 text-xs text-destructive'>再次点击“确认删除”后将不可恢复。</p>
                  ) : null}
                </div>

                <div className='flex shrink-0 items-center gap-2'>
                  <Button
                    disabled={isBusy}
                    onClick={() => void handleRestore(session.id)}
                    size='sm'
                    type='button'
                    variant='outline'
                  >
                    {isRestoreBusy ? (
                      <Loader2 className='mr-1 h-3.5 w-3.5 animate-spin' />
                    ) : (
                      <ArchiveRestore className='mr-1 h-3.5 w-3.5' />
                    )}
                    恢复
                  </Button>
                  <Button
                    className={isConfirmingPurge ? '' : 'text-destructive'}
                    disabled={isBusy && !isPurgeBusy}
                    onClick={() => void handlePurge(session.id)}
                    size='sm'
                    type='button'
                    variant={isConfirmingPurge ? 'destructive' : 'ghost'}
                  >
                    {isPurgeBusy ? (
                      <Loader2 className='mr-1 h-3.5 w-3.5 animate-spin' />
                    ) : (
                      <Trash2 className='mr-1 h-3.5 w-3.5' />
                    )}
                    {isPurgeBusy ? '删除中...' : isConfirmingPurge ? '确认删除' : '彻底删除'}
                  </Button>
                </div>
              </div>
            </div>
          )
        })}
      </CardContent>
    </Card>
  )
}
