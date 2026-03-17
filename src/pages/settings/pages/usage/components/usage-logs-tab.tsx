import { Loader2 } from 'lucide-react'

import { Badge } from '@/components/ui/badge'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import type { UsageLogItem, UsageLogsPayload } from '@/lib/types'
import { SETTINGS_PANEL_CLASSNAME } from '@/pages/settings/lib/ui'
import { cn } from '@/lib/utils'

import {
  PaginationBar,
  formatDateTime,
  formatDuration,
  formatNumber,
  statusBadgeClass,
  statusLabel,
  statusOptions,
  type UsageModelOption,
} from './usage-shared'

export function UsageLogsTab({
  loading,
  logsData,
  modelOptions,
  logModelId,
  onLogModelIdChange,
  logStatus,
  onLogStatusChange,
  detailLoadingTurnId,
  onOpenDetail,
  onPrevPage,
  onNextPage,
  currentPage,
}: {
  loading: boolean
  logsData: UsageLogsPayload | null
  modelOptions: UsageModelOption[]
  logModelId: string
  onLogModelIdChange: (value: string) => void
  logStatus: string
  onLogStatusChange: (value: string) => void
  detailLoadingTurnId: string | null
  onOpenDetail: (item: UsageLogItem) => void
  onPrevPage: () => void
  onNextPage: () => void
  currentPage: number
}) {
  return (
    <div className='space-y-4 pt-3'>
      <div className='grid gap-2 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]'>
        <Select onValueChange={(value) => onLogModelIdChange(value ?? 'all')} value={logModelId}>
          <SelectTrigger className='w-full'>
            <SelectValue placeholder='选择模型' />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value='all'>全部模型</SelectItem>
            {modelOptions.map((model) => (
              <SelectItem key={model.id} value={model.id}>
                {model.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <Select onValueChange={(value) => onLogStatusChange(value ?? 'all')} value={logStatus}>
          <SelectTrigger className='w-full'>
            <SelectValue placeholder='选择状态' />
          </SelectTrigger>
          <SelectContent>
            {statusOptions.map((item) => (
              <SelectItem key={item.value} value={item.value}>
                {item.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className='space-y-2'>
        {loading ? (
          <div className='flex items-center gap-2 rounded-xl px-3 py-5 text-sm text-muted-foreground'>
            <Loader2 className='h-4 w-4 animate-spin' /> 加载 Turn 日志中...
          </div>
        ) : logsData?.items.length ? (
          logsData.items.map((item) => (
            <button
              className={cn(
                SETTINGS_PANEL_CLASSNAME,
                'w-full space-y-3 text-left transition-colors hover:bg-accent/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50',
              )}
              key={item.turn_id}
              onClick={() => onOpenDetail(item)}
              type='button'
            >
              <div className='flex flex-wrap items-center gap-2'>
                <Badge className={cn(statusBadgeClass(item.status))}>
                  {statusLabel(item.status)}
                </Badge>
                <Badge className='bg-card text-foreground'>
                  {item.provider_name ?? '未绑定服务商'}
                </Badge>
                <Badge className='bg-card text-foreground'>
                  {item.model_name ?? item.model ?? '未绑定模型'}
                </Badge>
                {detailLoadingTurnId === item.turn_id ? (
                  <Loader2 className='ml-auto h-4 w-4 animate-spin text-muted-foreground' />
                ) : null}
              </div>

              <p className='line-clamp-2 text-sm text-foreground/90'>
                {item.user_message || '(空 Turn)'}
              </p>

              <div className='flex flex-wrap items-center gap-3 text-xs text-muted-foreground'>
                <span>{formatDateTime(item.started_at)}</span>
                <span>耗时 {formatDuration(item.duration_ms)}</span>
                <span>Step {formatNumber(item.step_count)}</span>
                <span>Token {formatNumber(item.total_tokens)}</span>
              </div>
            </button>
          ))
        ) : (
          <div className='rounded-xl px-3 py-5 text-sm text-muted-foreground'>
            当前筛选下暂无 Turn 日志。
          </div>
        )}
      </div>

      <PaginationBar
        hasMore={logsData?.page.has_more ?? false}
        loading={loading}
        onNext={onNextPage}
        onPrev={onPrevPage}
        page={logsData?.page.page ?? currentPage}
        total={logsData?.page.total ?? 0}
      />
    </div>
  )
}
