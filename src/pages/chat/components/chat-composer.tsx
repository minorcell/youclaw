import { useRef } from 'react'
import { SendHorizonal, Square } from 'lucide-react'

import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Textarea } from '@/components/ui/textarea'
import type { ProviderProfile } from '@/lib/types'

const IME_ENTER_GUARD_WINDOW_MS = 40

interface ChatComposerProps {
  input: string
  providers: ProviderProfile[]
  selectedProviderId: string | null
  onInputChange: (value: string) => void
  onSend: () => void
  isTurnRunning: boolean
  onCancelTurn: () => void
  onBindProvider: (providerProfileId: string | null) => void
}

export function ChatComposer({
  input,
  providers,
  selectedProviderId,
  onInputChange,
  onSend,
  isTurnRunning,
  onCancelTurn,
  onBindProvider,
}: ChatComposerProps) {
  const isComposingRef = useRef(false)
  const lastCompositionEndAtRef = useRef(0)

  const selectedProvider = providers.find((p) => p.id === selectedProviderId)
  const selectedModel = selectedProvider
    ? `${selectedProvider.name} / ${selectedProvider.model_name || selectedProvider.model}`
    : (selectedProviderId ?? '')

  return (
    <Card className='rounded-2xl border border-border/70 bg-background/95 py-0 shadow-[0_12px_40px_-26px_rgba(0,0,0,0.35)] backdrop-blur'>
      <div className='px-3 pt-3'>
        <Textarea
          className='rounded-xs min-h-16 max-h-60 resize-none border-0 bg-transparent p-0 text-[16px] leading-[1.4] shadow-none focus-visible:ring-0 overflow-y-auto dark:bg-transparent'
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

      <div className='flex items-center justify-end gap-3 px-3 pb-2'>
        <div className='flex items-center gap-2'>
          <Select
            onValueChange={(value) => onBindProvider(value)}
            value={selectedProviderId ?? providers[0]?.id ?? ''}
          >
            <SelectTrigger className='h-9 min-w-42.5 rounded-full border-border bg-muted/70 px-3 text-[13px]'>
              <SelectValue placeholder='选择模型'>{selectedModel}</SelectValue>
            </SelectTrigger>
            <SelectContent>
              {providers.map((provider) => (
                <SelectItem key={provider.id} value={provider.id}>
                  {provider.name} / {provider.model_name || provider.model}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button
            className={
              isTurnRunning
                ? 'h-9 w-9 rounded-full bg-destructive text-destructive-foreground hover:bg-destructive/90'
                : 'h-9 w-9 rounded-full bg-primary text-primary-foreground hover:bg-primary/90'
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
