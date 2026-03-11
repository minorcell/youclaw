import { History, MoreHorizontal, Pencil, Settings2, Trash2 } from 'lucide-react'
import { useMemo, useState } from 'react'

import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'
import type { ChatSession, ProviderProfile } from '@/lib/types'

interface SessionSidebarProps {
  sessions: ChatSession[]
  providers: ProviderProfile[]
  activeSessionId: string | null
  activeView?: 'chat' | 'settings'
  onCreateSession: () => void
  onDeleteSession: (sessionId: string) => void
  onRenameSession: (sessionId: string, title: string) => Promise<void>
  onSelectSession: (sessionId: string) => void
  onOpenSettings: () => void
}

const SESSION_TITLE_MAX_LENGTH = 48

export function SessionSidebar({
  sessions,
  providers,
  activeSessionId,
  activeView = 'chat',
  onCreateSession,
  onDeleteSession,
  onRenameSession,
  onSelectSession,
  onOpenSettings,
}: SessionSidebarProps) {
  const [renameSessionId, setRenameSessionId] = useState<string | null>(null)
  const [renameTitle, setRenameTitle] = useState('')
  const [renameBusy, setRenameBusy] = useState(false)

  const renameTargetSession = useMemo(
    () => sessions.find((session) => session.id === renameSessionId) ?? null,
    [sessions, renameSessionId],
  )

  const normalizedRenameTitle = renameTitle.trim()
  const canConfirmRename =
    renameTargetSession !== null &&
    normalizedRenameTitle.length > 0 &&
    normalizedRenameTitle !== renameTargetSession.title

  function closeRenameDialog() {
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
      // keep dialog open so user can retry or cancel
    } finally {
      setRenameBusy(false)
    }
  }

  return (
    <TooltipProvider delay={200}>
      <aside className='flex h-full flex-col overflow-hidden'>
        <div className='mt-2 min-h-0 flex-1 space-y-3 overflow-y-auto px-2 pb-4'>
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
                    onClick={() => onSelectSession(session.id)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault()
                        onSelectSession(session.id)
                      }
                    }}
                    role='button'
                    tabIndex={0}
                  >
                    <div className='min-w-0 flex-1 text-left'>
                      <p className='truncate text-sm font-medium text-foreground'>
                        {session.title}
                      </p>
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
                            setRenameSessionId(session.id)
                            setRenameTitle(session.title)
                          }}
                        >
                          <Pencil className='h-4 w-4' />
                          重命名
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          onClick={(event) => {
                            event.stopPropagation()
                            onDeleteSession(session.id)
                          }}
                          variant='destructive'
                        >
                          <Trash2 className='h-4 w-4' />
                          删除
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                ))
              )}
            </div>
          </Card>
        </div>

        <div className='px-3 py-3'>
          <div className='flex items-center justify-between rounded-2xl bg-card/60 px-3 py-2'>
            <div className='flex items-center gap-2'>
              <div>
                <p className='text-xs text-muted-foreground'>{providers.length} 个模型配置</p>
              </div>
            </div>
            <Button
              className={cn(
                'h-8 w-8 rounded-lg',
                activeView === 'settings' && 'bg-accent text-accent-foreground',
              )}
              onClick={onOpenSettings}
              size='icon'
              type='button'
              variant='ghost'
            >
              <Settings2 className='h-4 w-4' />
            </Button>
          </div>
        </div>
      </aside>

      <Dialog
        onOpenChange={(open) => {
          if (!open) {
            closeRenameDialog()
          }
        }}
        open={renameTargetSession !== null}
      >
        <DialogContent className='max-w-sm' showCloseButton={false}>
          <DialogTitle>确认重命名</DialogTitle>
          <div className='space-y-3'>
            <Label htmlFor='rename-session-input'>会话名称</Label>
            <Input
              id='rename-session-input'
              maxLength={SESSION_TITLE_MAX_LENGTH}
              onChange={(event) => setRenameTitle(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault()
                  void confirmRename()
                }
              }}
              value={renameTitle}
            />
            <p className='text-xs text-muted-foreground'>
              将会把会话重命名为新标题，确认后立即生效。
            </p>
          </div>
          <div className='flex justify-end gap-2'>
            <Button
              disabled={renameBusy}
              onClick={closeRenameDialog}
              type='button'
              variant='outline'
            >
              取消
            </Button>
            <Button
              disabled={!canConfirmRename || renameBusy}
              onClick={() => void confirmRename()}
              type='button'
            >
              {renameBusy ? '保存中...' : '确认重命名'}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
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
