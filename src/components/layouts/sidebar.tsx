import { ArrowLeft, History, MoreHorizontal, Pencil, Settings2, Trash2 } from 'lucide-react'
import { useMemo, useState } from 'react'

import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { settingsSectionMeta, settingsSections } from '@/pages/settings/sections'
import type { SettingsSection } from '@/store/settings-store'
import { cn } from '@/lib/utils'
import type { ChatSession, ProviderProfile } from '@/lib/types'

interface AppSidebarProps {
  sessions: ChatSession[]
  providers: ProviderProfile[]
  activeSessionId: string | null
  activeView?: 'chat' | 'settings'
  activeSettingsSection?: SettingsSection | null
  onCreateSession: () => void
  onDeleteSession: (sessionId: string) => Promise<void>
  onRenameSession: (sessionId: string, title: string) => Promise<void>
  onSelectSession: (sessionId: string) => void
  onSelectSettingsSection: (section: SettingsSection) => void
  onOpenChat: () => void
  onOpenSettings: () => void
}

const SESSION_TITLE_MAX_LENGTH = 48

export function AppSidebar({
  sessions,
  providers,
  activeSessionId,
  activeView = 'chat',
  activeSettingsSection = null,
  onCreateSession,
  onDeleteSession,
  onRenameSession,
  onSelectSession,
  onSelectSettingsSection,
  onOpenChat,
  onOpenSettings,
}: AppSidebarProps) {
  const [renameSessionId, setRenameSessionId] = useState<string | null>(null)
  const [renameTitle, setRenameTitle] = useState('')
  const [renameBusy, setRenameBusy] = useState(false)
  const [confirmDeleteSessionId, setConfirmDeleteSessionId] = useState<string | null>(null)
  const [deleteBusySessionId, setDeleteBusySessionId] = useState<string | null>(null)

  const renameTargetSession = useMemo(
    () => sessions.find((session) => session.id === renameSessionId) ?? null,
    [sessions, renameSessionId],
  )

  const normalizedRenameTitle = renameTitle.trim()
  const canConfirmRename =
    renameTargetSession !== null &&
    normalizedRenameTitle.length > 0 &&
    normalizedRenameTitle !== renameTargetSession.title

  function cancelRename() {
    if (renameBusy) return
    setRenameSessionId(null)
    setRenameTitle('')
  }

  async function confirmRename() {
    if (!renameTargetSession || !canConfirmRename || renameBusy) return
    setRenameBusy(true)
    try {
      await onRenameSession(renameTargetSession.id, normalizedRenameTitle)
      setRenameSessionId(null)
      setRenameTitle('')
    } catch {
      // keep inline editor open so user can retry or cancel
    } finally {
      setRenameBusy(false)
    }
  }

  async function confirmArchive(sessionId: string) {
    if (deleteBusySessionId) return
    setDeleteBusySessionId(sessionId)
    try {
      await onDeleteSession(sessionId)
      setConfirmDeleteSessionId(null)
    } catch {
      // Keep the confirmation state so user can retry.
    } finally {
      setDeleteBusySessionId(null)
    }
  }

  return (
    <TooltipProvider delay={200}>
      <aside className='flex h-full flex-col overflow-hidden'>
        <div className='px-4 pt-2'>
          <p className='text-sm font-semibold tracking-[0.12em] text-foreground/75'>youclaw</p>
        </div>

        <div className='no-scrollbar mt-1 min-h-0 flex-1 space-y-3 overflow-y-auto px-2 pb-4'>
          {activeView === 'settings' ? (
            <Card className='bg-card/60 py-0'>
              <div className='flex items-center gap-2 px-4 py-3 text-sm text-muted-foreground'>
                <Settings2 className='h-4 w-4' />
                设置目录
              </div>
              <div className='space-y-1.5 px-2 pb-2'>
                {settingsSections.map((sectionId) => {
                  const item = settingsSectionMeta[sectionId]
                  const Icon = item.icon
                  const isActive = activeSettingsSection === sectionId
                  return (
                    <button
                      aria-current={isActive ? 'page' : undefined}
                      className={cn(
                        'w-full rounded-xl px-3 py-2 text-left transition-colors',
                        isActive
                          ? 'bg-accent/55 text-accent-foreground'
                          : 'bg-transparent text-muted-foreground hover:bg-accent/30 hover:text-foreground',
                      )}
                      key={sectionId}
                      onClick={() => onSelectSettingsSection(sectionId)}
                      type='button'
                    >
                      <div className='flex items-center gap-2'>
                        <Icon className='h-4 w-4' />
                        <span className='text-sm font-medium'>{item.label}</span>
                      </div>
                    </button>
                  )
                })}
              </div>
            </Card>
          ) : (
            <Card className='bg-card/60 py-0'>
              <div className='flex items-center justify-between px-4 py-3 text-sm text-muted-foreground'>
                <div className='flex items-center gap-2'>
                  <History className='h-4 w-4' />
                  最近会话
                </div>
                <Tooltip>
                  <TooltipTrigger>
                    <PlusBadge onClick={onCreateSession} />
                  </TooltipTrigger>
                  <TooltipContent>新建会话</TooltipContent>
                </Tooltip>
              </div>
              <div className='space-y-2 px-2 pb-2'>
                {sessions.length === 0 ? (
                  <div className='rounded-xl px-3 py-4 text-sm text-muted-foreground'>
                    还没有会话...
                  </div>
                ) : (
                  sessions.map((session) => (
                    <div
                      className={cn(
                        'group flex items-center gap-2 rounded-xl px-3 py-2 transition-colors',
                        session.id === activeSessionId
                          ? 'bg-accent/55'
                          : 'bg-transparent hover:bg-muted/45',
                      )}
                      key={session.id}
                      onClick={() => {
                        if (renameSessionId === session.id) return
                        setConfirmDeleteSessionId(null)
                        onSelectSession(session.id)
                      }}
                      onKeyDown={(event) => {
                        if (renameSessionId === session.id) return
                        if (event.key === 'Enter' || event.key === ' ') {
                          event.preventDefault()
                          setConfirmDeleteSessionId(null)
                          onSelectSession(session.id)
                        }
                      }}
                      role='button'
                      tabIndex={0}
                    >
                      <div className='min-w-0 flex-1 text-left'>
                        {renameSessionId === session.id ? (
                          <Input
                            autoFocus
                            className='h-7 text-sm font-medium shadow-none focus-visible:ring-0 focus-visible:shadow-none'
                            disabled={renameBusy}
                            maxLength={SESSION_TITLE_MAX_LENGTH}
                            onBlur={() => {
                              if (renameBusy) return
                              if (!canConfirmRename) {
                                cancelRename()
                                return
                              }
                              void confirmRename()
                            }}
                            onChange={(event) => setRenameTitle(event.target.value)}
                            onClick={(event) => {
                              event.stopPropagation()
                            }}
                            onFocus={(event) => {
                              event.stopPropagation()
                              event.target.select()
                            }}
                            onKeyDown={(event) => {
                              event.stopPropagation()
                              if (event.key === 'Enter') {
                                event.preventDefault()
                                event.currentTarget.blur()
                              }
                              if (event.key === 'Escape') {
                                event.preventDefault()
                                cancelRename()
                              }
                            }}
                            value={renameTitle}
                          />
                        ) : (
                          <p className='truncate text-sm font-medium text-foreground'>
                            {session.title}
                          </p>
                        )}
                      </div>
                      <DropdownMenu modal={false}>
                        <DropdownMenuTrigger
                          className={cn(
                            'inline-flex h-7 w-7 items-center justify-center rounded-lg text-muted-foreground transition-opacity',
                            'opacity-0 pointer-events-none group-hover:opacity-100 group-hover:pointer-events-auto',
                            'data-open:opacity-100 data-open:pointer-events-auto',
                          )}
                          onClick={(event) => {
                            event.stopPropagation()
                          }}
                          onKeyDown={(event) => {
                            event.stopPropagation()
                          }}
                          type='button'
                        >
                          <MoreHorizontal className='h-3.5 w-3.5' />
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align='end' className='w-36' sideOffset={8}>
                          <DropdownMenuItem
                            onClick={(event) => {
                              event.stopPropagation()
                              if (renameBusy) return
                              setConfirmDeleteSessionId(null)
                              setRenameSessionId(session.id)
                              setRenameTitle(session.title)
                            }}
                          >
                            <Pencil className='h-4 w-4' />
                            重命名
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            closeOnClick={confirmDeleteSessionId === session.id}
                            disabled={
                              deleteBusySessionId !== null && deleteBusySessionId !== session.id
                            }
                            onClick={(event) => {
                              event.stopPropagation()
                              if (deleteBusySessionId === session.id) {
                                return
                              }
                              if (confirmDeleteSessionId !== session.id) {
                                setConfirmDeleteSessionId(session.id)
                                return
                              }
                              void confirmArchive(session.id)
                            }}
                            variant='destructive'
                          >
                            <Trash2 className='h-4 w-4' />
                            {deleteBusySessionId === session.id
                              ? '归档中...'
                              : confirmDeleteSessionId === session.id
                                ? '确认归档'
                                : '归档'}
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </div>
                  ))
                )}
              </div>
            </Card>
          )}
        </div>

        <div className='px-3 py-3'>
          {activeView === 'settings' ? (
            <div className='flex items-center justify-between rounded-2xl bg-card/60 px-3 py-2'>
              <div>
                <p className='text-sm font-medium text-foreground'>返回会话</p>
                <p className='text-xs text-muted-foreground'>回到聊天列表</p>
              </div>
              <Button onClick={onOpenChat} size='icon' type='button' variant='ghost'>
                <ArrowLeft className='h-4 w-4' />
              </Button>
            </div>
          ) : (
            <div className='flex items-center justify-between rounded-2xl bg-card/60 px-3 py-2'>
              <div className='flex items-center gap-2'>
                <div>
                  <p className='text-xs text-muted-foreground'>{providers.length} 个模型配置</p>
                </div>
              </div>
              <Button
                className='h-8 w-8 rounded-lg'
                onClick={onOpenSettings}
                size='icon'
                type='button'
                variant='ghost'
              >
                <Settings2 className='h-4 w-4' />
              </Button>
            </div>
          )}
        </div>
      </aside>
    </TooltipProvider>
  )
}

function PlusBadge({ onClick }: { onClick?: () => void }) {
  return (
    <button
      aria-label='新建会话'
      className='flex h-5 w-5 items-center justify-center rounded-md bg-background text-muted-foreground transition hover:bg-muted/70'
      onClick={onClick}
      type='button'
    >
      +
    </button>
  )
}
