import { Badge } from '@/components/ui/badge'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import type { UsageLogDetailPayload, UsageLogItem } from '@/lib/types'
import { cn } from '@/lib/utils'

import {
  formatDateTime,
  formatDuration,
  formatNumber,
  statusBadgeClass,
  statusLabel,
} from './usage-shared'

export function UsageLogDetailDialog({
  open,
  onOpenChange,
  item,
  detail,
  loading,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  item: UsageLogItem | null
  detail: UsageLogDetailPayload | null
  loading: boolean
}) {
  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent className='max-w-3xl p-0 sm:max-w-3xl' showCloseButton>
        <div className='space-y-4 p-4'>
          <DialogHeader>
            <DialogTitle>Turn 详情</DialogTitle>
            <DialogDescription>
              {item ? formatDateTime(item.started_at) : '查看本次会话的工具调用与消耗明细。'}
            </DialogDescription>
          </DialogHeader>

          {item ? (
            <>
              <div className='flex flex-wrap items-center gap-2'>
                <Badge className={cn(statusBadgeClass(item.status))}>{statusLabel(item.status)}</Badge>
                <Badge className='bg-card text-foreground'>
                  {item.provider_name ?? '未绑定服务商'}
                </Badge>
                <Badge className='bg-card text-foreground'>
                  {item.model_name ?? item.model ?? '未绑定模型'}
                </Badge>
              </div>

              <div className='rounded-xl bg-muted/30 p-3 text-sm text-foreground/90'>
                {item.user_message || '(空 Turn)'}
              </div>

              <div className='grid gap-2 sm:grid-cols-2 xl:grid-cols-4'>
                <StatTile label='耗时' value={formatDuration(item.duration_ms)} />
                <StatTile label='Step' value={formatNumber(item.step_count)} />
                <StatTile
                  label='输入 / 输出'
                  value={`${formatNumber(item.input_tokens)} / ${formatNumber(item.output_tokens)}`}
                />
                <StatTile label='总 Token' value={formatNumber(item.total_tokens)} />
              </div>

              <div className='space-y-2'>
                <p className='text-sm font-medium text-foreground'>工具调用</p>

                {loading ? (
                  <div className='rounded-xl bg-background/80 px-3 py-6 text-sm text-muted-foreground'>
                    正在加载详情...
                  </div>
                ) : detail?.tools.length ? (
                  <div className='no-scrollbar max-h-[50vh] space-y-2 overflow-y-auto pr-1'>
                    {detail.tools.map((tool) => (
                      <div className='rounded-xl bg-background/80 p-3' key={tool.id}>
                        <div className='flex flex-wrap items-center gap-2'>
                          <Badge className='bg-card text-foreground'>{tool.tool_name}</Badge>
                          {tool.tool_action ? (
                            <Badge className='bg-card text-foreground'>{tool.tool_action}</Badge>
                          ) : null}
                          <Badge
                            className={cn(
                              tool.is_error
                                ? 'bg-destructive/10 text-destructive'
                                : 'bg-primary/15 text-primary',
                            )}
                          >
                            {tool.status}
                          </Badge>
                        </div>
                        <div className='mt-2 flex flex-wrap items-center gap-3 text-xs text-muted-foreground'>
                          <span>{formatDuration(tool.duration_ms)}</span>
                          <span>{formatDateTime(tool.created_at)}</span>
                        </div>
                        {Object.keys(tool.args_json ?? {}).length > 0 ? (
                          <div className='mt-3 space-y-1.5'>
                            <p className='text-xs font-medium text-muted-foreground'>参数</p>
                            <pre
                              className='no-scrollbar max-h-56 overflow-auto rounded-lg bg-muted/35 p-3 text-[11px] leading-5 text-foreground/85'
                              data-allow-text-selection='true'
                            >
                              {JSON.stringify(tool.args_json, null, 2)}
                            </pre>
                          </div>
                        ) : null}
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className='rounded-xl bg-background/80 px-3 py-6 text-sm text-muted-foreground'>
                    当前 Turn 没有可展示的工具详情。
                  </div>
                )}
              </div>
            </>
          ) : null}
        </div>
      </DialogContent>
    </Dialog>
  )
}

function StatTile({ label, value }: { label: string; value: string }) {
  return (
    <div className='rounded-xl bg-background/80 p-3'>
      <p className='text-xs text-muted-foreground'>{label}</p>
      <p className='mt-1 text-sm font-medium text-foreground'>{value}</p>
    </div>
  )
}
