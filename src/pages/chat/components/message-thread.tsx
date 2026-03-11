import { memo } from 'react'
import { code } from '@streamdown/code'
import { Bot, TriangleAlert } from 'lucide-react'
import { Streamdown } from 'streamdown'

import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'
import type { StepRenderUnit, ToolRenderUnit, TurnRenderUnit } from '@/lib/types'

const streamdownPlugins = { code }

interface MessageThreadProps {
  turns: TurnRenderUnit[]
  providerLabel?: string
}

function ToolBlock({ tool }: { tool: ToolRenderUnit }) {
  return (
    <details className='overflow-hidden rounded-2xl border border-border/70 bg-muted/30'>
      <summary className='flex cursor-pointer list-none items-center justify-between gap-2 px-3 py-2 text-xs [&::-webkit-details-marker]:hidden'>
        <div className='flex items-center gap-2'>
          <Badge variant='secondary'>Tool Call</Badge>
          <Badge>{tool.toolName}</Badge>
          {tool.result ? (
            <Badge variant={tool.result.is_error ? 'destructive' : 'default'}>
              {tool.result.is_error ? 'Error' : 'OK'}
            </Badge>
          ) : tool.isLive ? (
            <Badge variant='secondary'>Running</Badge>
          ) : null}
        </div>
        <span className='text-muted-foreground'>点击展开</span>
      </summary>
      <pre className='no-scrollbar max-h-72 overflow-auto p-3 text-[11px] leading-5 text-foreground/90'>
        {JSON.stringify(tool.argsJson, null, 2)}
      </pre>
      {tool.result ? (
        <div className='border-t border-border/70'>
          <div className='flex items-center gap-2 px-3 pt-2 text-xs'>
            <Badge variant='secondary'>Tool Result</Badge>
          </div>
          <pre className='no-scrollbar max-h-72 overflow-auto p-3 text-[11px] leading-5 text-foreground/90'>
            {JSON.stringify(tool.result.output_json, null, 2)}
          </pre>
        </div>
      ) : null}
    </details>
  )
}

function StepBlock({ step }: { step: StepRenderUnit }) {
  return (
    <div>
      {step.reasoningText ? (
        <details className='mb-3 rounded-2xl border border-border/70 bg-muted/30 px-4 py-3 text-sm'>
          <summary className='cursor-pointer text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground'>
            Model reasoning
          </summary>
          <Streamdown
            className='mt-3 text-sm leading-7 text-muted-foreground'
            controls={false}
            isAnimating={step.isLive}
            mode={step.isLive ? undefined : 'static'}
            plugins={streamdownPlugins}
          >
            {step.reasoningText}
          </Streamdown>
        </details>
      ) : null}
      {step.outputText ? (
        <Streamdown
          caret={step.isLive ? 'block' : undefined}
          className='text-base leading-8 text-foreground'
          controls={false}
          isAnimating={step.isLive}
          mode={step.isLive ? undefined : 'static'}
          plugins={streamdownPlugins}
        >
          {step.outputText}
        </Streamdown>
      ) : null}
      {step.tools.length > 0 ? (
        <div className='mt-3 space-y-2'>
          {step.tools.map((tool) => (
            <ToolBlock key={tool.callId} tool={tool} />
          ))}
        </div>
      ) : null}
    </div>
  )
}

function AgentResponse({
  steps,
  providerLabel,
  error,
}: {
  steps: StepRenderUnit[]
  providerLabel: string
  error?: string
}) {
  const hasContent = steps.some(
    (step) => step.outputText || step.reasoningText || step.tools.length > 0,
  )
  if (!hasContent && !error) return null

  const isMultiStep = steps.length > 1

  return (
    <article className='max-w-[76ch]'>
      <div className='mb-2 flex items-center gap-2 text-sm text-muted-foreground'>
        <Bot className='h-4 w-4' />
        <span className='font-medium'>{providerLabel}</span>
      </div>
      {steps.map((step, index) => (
        <div key={step.step}>
          {isMultiStep && index > 0 ? (
            <div className='my-4 flex items-center gap-3'>
              <div className='h-px flex-1 bg-border/30' />
              <span className='text-[10px] font-medium uppercase tracking-[0.2em] text-muted-foreground/50'>
                step {index + 1}
              </span>
              <div className='h-px flex-1 bg-border/30' />
            </div>
          ) : null}
          <StepBlock step={step} />
        </div>
      ))}
      {error ? (
        <Card className='mt-3 border-destructive/20 bg-destructive/10 px-4 py-3 text-sm text-destructive shadow-none'>
          <div className='flex items-start gap-2'>
            <TriangleAlert className='mt-0.5 h-4 w-4' />
            <span>{error}</span>
          </div>
        </Card>
      ) : null}
    </article>
  )
}

function MessageThreadView({ turns, providerLabel = 'Agent' }: MessageThreadProps) {
  return (
    <div className='space-y-6'>
      {turns.map((turn) => (
        <div key={turn.turnId} className='space-y-6'>
          {turn.userText ? (
            <div className='flex justify-end'>
              <Streamdown
                className='max-w-[68ch] text-sm leading-7 text-foreground'
                controls={false}
                mode='static'
                plugins={streamdownPlugins}
              >
                {turn.userText}
              </Streamdown>
            </div>
          ) : null}
          <AgentResponse error={turn.error} providerLabel={providerLabel} steps={turn.steps} />
        </div>
      ))}
    </div>
  )
}

export const MessageThread = memo(MessageThreadView)
MessageThread.displayName = 'MessageThread'
