import { History, Settings2, Trash2 } from 'lucide-react'

import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '../ui/tooltip'
import type { ChatSession, ProviderProfile } from '@/lib/types'
import { cn } from '@/lib/utils'

interface SessionSidebarProps {
  sessions: ChatSession[]
  providers: ProviderProfile[]
  activeSessionId: string | null
  activeView?: 'chat' | 'settings'
  onCreateSession: () => void
  onDeleteSession: (sessionId: string) => void
  onSelectSession: (sessionId: string) => void
  onOpenSettings: () => void
}

export function SessionSidebar({
  sessions,
  providers,
  activeSessionId,
  activeView = 'chat',
  onCreateSession,
  onDeleteSession,
  onSelectSession,
  onOpenSettings,
}: SessionSidebarProps) {
  return (
    <TooltipProvider delay={200}>
      <aside className='flex h-full flex-col overflow-hidden'>
        <div className='px-5 pb-4 pt-6'>
          <div className='flex items-center justify-between gap-2'>
            <p className='text-[1.5rem] font-serif leading-none tracking-tight text-[#224c37]'>
              包工头 BgtClaw
            </p>
          </div>
        </div>

        <div className='mt-2 min-h-0 flex-1 space-y-3 overflow-y-auto px-2 pb-4'>
          <Card className='border border-border/70 bg-card/60 py-0'>
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
                <div className='rounded-xl border border-dashed border-border/70 px-3 py-4 text-sm text-muted-foreground'>
                  还没有会话...
                </div>
              ) : (
                sessions.map((session) => (
                  <div
                    className={cn(
                      'group flex items-center gap-2 rounded-xl border px-3 py-2',
                      session.id === activeSessionId
                        ? 'border-border bg-background'
                        : 'border-transparent hover:border-border/60 hover:bg-background/70',
                    )}
                    key={session.id}
                  >
                    <button
                      className='min-w-0 flex-1 text-left'
                      onClick={() => onSelectSession(session.id)}
                      type='button'
                    >
                      <p className='truncate text-sm font-medium text-foreground'>
                        {session.title}
                      </p>
                    </button>
                    <Button
                      className={cn(
                        'h-7 w-7 rounded-lg text-muted-foreground',
                        session.id === activeSessionId
                          ? 'opacity-100'
                          : 'opacity-0 group-hover:opacity-100',
                      )}
                      onClick={(event) => {
                        event.stopPropagation()
                        onDeleteSession(session.id)
                      }}
                      size='icon-sm'
                      type='button'
                      variant='ghost'
                    >
                      <Trash2 className='h-3.5 w-3.5' />
                    </Button>
                  </div>
                ))
              )}
            </div>
          </Card>
        </div>

        <div className='px-3 py-3'>
          <div className='flex items-center justify-between rounded-2xl border border-border/70 bg-card/60 px-3 py-2'>
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
    </TooltipProvider>
  )
}

function PlusBadge({ onClick }: { onClick?: () => void }) {
  return (
    <button
      aria-label='新建会话'
      className='flex h-5 w-5 items-center justify-center rounded-md border border-border/80 bg-background text-muted-foreground transition hover:bg-muted/70'
      onClick={onClick}
      type='button'
    >
      +
    </button>
  )
}
