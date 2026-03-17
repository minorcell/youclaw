import { Clock3, FolderCode, Hammer } from 'lucide-react'

import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'
import type { TurnViewState } from '@/lib/types'

import { ToolApprovalCard } from './tool-approval-card'

interface TimelinePanelProps {
  turn: TurnViewState | null
  onResolveApproval: (approvalId: string, approved: boolean) => void
}

export function TimelinePanel({ turn, onResolveApproval }: TimelinePanelProps) {
  const timelineItems = turn ? [...turn.timeline, ...Object.values(turn.liveStepsById)] : []

  return (
    <Card className='flex h-full flex-col overflow-hidden'>
      <div className='border-b border-border/70 p-5'>
        <p className='text-xs uppercase tracking-[0.24em] text-muted-foreground'>Tool Timeline</p>
        <h2 className='mt-2 text-xl font-semibold'>
          {turn ? `Turn ${turn.turn.id.slice(0, 8)}` : 'No active turn'}
        </h2>
      </div>
      <div className='flex-1 space-y-3 overflow-y-auto p-4'>
        {turn ? (
          timelineItems.map((item) => (
            <Card className='p-4' key={item.id}>
              {item.kind === 'step' ? (
                <div className='space-y-3'>
                  <div className='flex items-center gap-2'>
                    <Clock3 className='h-4 w-4 text-muted-foreground' />
                    <Badge>step {item.step}</Badge>
                    <span className='text-xs text-muted-foreground'>{item.status}</span>
                  </div>
                  <pre className='whitespace-pre-wrap wrap-break-word text-xs leading-6 text-muted-foreground'>
                    {item.outputText || 'Waiting for model output...'}
                  </pre>
                  {item.reasoningText ? (
                    <pre className='whitespace-pre-wrap wrap-break-word rounded-2xl border border-border/60 bg-muted/40 p-3 text-[11px] leading-5 text-muted-foreground'>
                      {item.reasoningText}
                    </pre>
                  ) : null}
                  {item.usage ? (
                    <p className='text-xs text-muted-foreground'>
                      tokens: {item.usage.input_tokens}/{item.usage.output_tokens}/
                      {item.usage.reasoning_tokens}/{item.usage.total_tokens}
                    </p>
                  ) : null}
                </div>
              ) : (
                <div className='space-y-3'>
                  <div className='flex items-center gap-2'>
                    <FolderCode className='h-4 w-4 text-muted-foreground' />
                    <Badge>{item.toolCall.tool_name}</Badge>
                    <span className='text-xs text-muted-foreground'>step {item.step}</span>
                  </div>
                  <pre className='overflow-x-auto rounded-2xl bg-secondary/60 p-3 text-[11px] leading-5 text-secondary-foreground'>
                    {JSON.stringify(item.toolCall.args_json, null, 2)}
                  </pre>
                  {item.approval ? (
                    <ToolApprovalCard
                      approval={item.approval}
                      onResolveApproval={onResolveApproval}
                    />
                  ) : null}
                  {item.toolResult ? (
                    <div>
                      <div className='mb-2 flex items-center gap-2 text-xs text-muted-foreground'>
                        <Hammer className='h-4 w-4' />
                        {item.durationMs ? `${item.durationMs} ms` : 'finished'}
                      </div>
                      <pre className='overflow-x-auto rounded-2xl bg-secondary/60 p-3 text-[11px] leading-5 text-secondary-foreground'>
                        {JSON.stringify(item.toolResult.output_json, null, 2)}
                      </pre>
                    </div>
                  ) : null}
                </div>
              )}
            </Card>
          ))
        ) : (
          <Card className='p-5 text-sm text-muted-foreground'>
            发起一次对话后，这里会显示 step、tool 调用和工具审批。
          </Card>
        )}
      </div>
    </Card>
  )
}
