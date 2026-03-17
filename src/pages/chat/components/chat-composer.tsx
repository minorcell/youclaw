import { useRef } from 'react'
import {
  Check,
  ChevronDown,
  Loader2,
  SendHorizonal,
  Shield,
  ShieldCheck,
  Square,
} from 'lucide-react'

import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Textarea } from '@/components/ui/textarea'
import {
  labelForSessionApprovalMode,
  sessionApprovalModeOptions,
} from '@/lib/session-approval-mode'
import type { ProviderProfile, SessionApprovalMode } from '@/lib/types'
import { cn } from '@/lib/utils'

const IME_ENTER_GUARD_WINDOW_MS = 40
const COMPOSER_MENU_TRIGGER_CLASSNAME =
  'inline-flex h-8 min-w-0 items-center gap-1.5 rounded-full bg-transparent px-3 text-[13px] text-foreground transition-colors hover:bg-muted/70 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-60'

interface ChatComposerProps {
  input: string
  providers: ProviderProfile[]
  selectedProviderId: string | null
  approvalMode: SessionApprovalMode
  approvalModeBusy: boolean
  onInputChange: (value: string) => void
  onSend: () => void
  isTurnRunning: boolean
  onCancelTurn: () => void
  onBindProvider: (providerProfileId: string | null) => void
  onApprovalModeChange: (approvalMode: SessionApprovalMode) => void
}

export function ChatComposer({
  input,
  providers,
  selectedProviderId,
  approvalMode,
  approvalModeBusy,
  onInputChange,
  onSend,
  isTurnRunning,
  onCancelTurn,
  onBindProvider,
  onApprovalModeChange,
}: ChatComposerProps) {
  const isComposingRef = useRef(false)
  const lastCompositionEndAtRef = useRef(0)

  const selectedProvider =
    providers.find((p) => p.id === selectedProviderId) ?? providers[0] ?? null
  const selectedModel = selectedProvider
    ? selectedProvider.model_name || selectedProvider.model
    : '选择模型'
  const approvalModeLabel = labelForSessionApprovalMode(approvalMode)
  const ApprovalModeIcon = approvalMode === 'full_access' ? ShieldCheck : Shield

  return (
    <Card className='rounded-2xl border border-border/70 bg-background/95 py-0 shadow-lg backdrop-blur'>
      <div className='px-3 pt-2.5'>
        <Textarea
          className='rounded-xs min-h-14 max-h-56 resize-none border-0 bg-transparent p-0 text-[15px] leading-[1.35] shadow-none focus-visible:ring-0 overflow-y-auto dark:bg-transparent'
          onChange={(event) => onInputChange(event.target.value)}
          onCompositionEnd={() => {
            isComposingRef.current = false
            lastCompositionEndAtRef.current = Date.now()
          }}
          onCompositionStart={() => {
            isComposingRef.current = true
          }}
          onKeyDown={(event) => {
            if (event.key !== 'Enter' || event.shiftKey) return

            const endedRecently =
              Date.now() - lastCompositionEndAtRef.current < IME_ENTER_GUARD_WINDOW_MS
            const isImeComposing =
              isComposingRef.current ||
              event.nativeEvent.isComposing ||
              event.nativeEvent.keyCode === 229

            if (isImeComposing || endedRecently) {
              return
            }

            event.preventDefault()
            if (isTurnRunning) return
            onSend()
          }}
          placeholder='输入消息...'
          value={input}
        />
      </div>

      <div className='flex min-w-0 items-center justify-between gap-3 px-3 pb-1.5'>
        <DropdownMenu modal={false}>
          <DropdownMenuTrigger
            className={COMPOSER_MENU_TRIGGER_CLASSNAME}
            disabled={approvalModeBusy}
            type='button'
          >
            {approvalModeBusy ? (
              <Loader2 className='h-3.5 w-3.5 animate-spin text-muted-foreground' />
            ) : (
              <ApprovalModeIcon
                className={cn(
                  'h-3.5 w-3.5',
                  approvalMode === 'full_access' ? 'text-destructive' : 'text-muted-foreground',
                )}
              />
            )}
            <span>{approvalModeLabel}</span>
            <ChevronDown className='h-3.5 w-3.5 text-muted-foreground' />
          </DropdownMenuTrigger>
          <DropdownMenuContent align='start' className='w-44 p-1'>
            {sessionApprovalModeOptions.map((item) => (
              <DropdownMenuItem
                className='gap-2 py-1.5'
                disabled={approvalModeBusy}
                key={item.value}
                onClick={() => onApprovalModeChange(item.value)}
              >
                <div className='min-w-0 flex-1'>
                  <div className='text-sm font-medium text-foreground'>{item.label}</div>
                  <div className='text-[11px] text-muted-foreground'>{item.description}</div>
                </div>
                {approvalMode === item.value ? (
                  <Check className='h-3.5 w-3.5 text-foreground' />
                ) : null}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>

        <div className='flex min-w-0 items-center gap-2'>
          <DropdownMenu modal={false}>
            <DropdownMenuTrigger
              className={cn(COMPOSER_MENU_TRIGGER_CLASSNAME, 'max-w-72')}
              type='button'
            >
              <span className='truncate'>{selectedModel}</span>
              <ChevronDown className='h-3.5 w-3.5 text-muted-foreground' />
            </DropdownMenuTrigger>
            <DropdownMenuContent align='end' className='w-64 p-1'>
              {providers.map((provider) => (
                <DropdownMenuItem
                  className='gap-2 py-1.5'
                  key={provider.id}
                  onClick={() => onBindProvider(provider.id)}
                >
                  <div className='min-w-0 flex-1'>
                    <div className='truncate text-sm font-medium text-foreground'>
                      {provider.model_name || provider.model}
                    </div>
                    <div className='truncate text-[11px] text-muted-foreground'>
                      {provider.name}
                    </div>
                  </div>
                  {selectedProvider?.id === provider.id ? (
                    <Check className='h-3.5 w-3.5 text-foreground' />
                  ) : null}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
          <Button
            className={
              isTurnRunning
                ? 'rounded-full bg-destructive text-destructive-foreground hover:bg-destructive/90'
                : 'rounded-full bg-primary text-primary-foreground hover:bg-primary/90'
            }
            onClick={isTurnRunning ? onCancelTurn : onSend}
            size='icon'
            type='button'
          >
            {isTurnRunning ? (
              <Square className='h-3.5 w-3.5' />
            ) : (
              <SendHorizonal className='h-4 w-4' />
            )}
          </Button>
        </div>
      </div>
    </Card>
  )
}
