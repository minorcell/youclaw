import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { UsageStatsRange } from '@/lib/types'

export type UsageTab = 'logs' | 'providers' | 'models' | 'tools'

export interface UsageModelOption {
  id: string
  label: string
}

export const DEFAULT_PAGE_SIZE = 20

export const rangeOptions: Array<{ value: UsageStatsRange; label: string }> = [
  { value: '24h', label: '24h' },
  { value: '7d', label: '7天' },
  { value: '30d', label: '30天' },
  { value: 'all', label: '全部' },
]

export const statusOptions: Array<{ value: string; label: string }> = [
  { value: 'all', label: '全部状态' },
  { value: 'running', label: '运行中' },
  { value: 'completed', label: '已完成' },
  { value: 'failed', label: '失败' },
  { value: 'cancelled', label: '已取消' },
]

export function formatNumber(value: number): string {
  return new Intl.NumberFormat('zh-CN').format(Math.max(0, value))
}

export function formatDateTime(value: string | null): string {
  if (!value) return '-'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleString('zh-CN', {
    hour12: false,
  })
}

export function formatDuration(value: number | null): string {
  if (value === null || value < 0) return '-'
  if (value < 1000) return `${value} ms`
  return `${(value / 1000).toFixed(2)} s`
}

export function statusLabel(status: string): string {
  switch (status) {
    case 'running':
      return '运行中'
    case 'completed':
      return '已完成'
    case 'failed':
      return '失败'
    case 'cancelled':
      return '已取消'
    default:
      return status
  }
}

export function statusBadgeClass(status: string): string {
  if (status === 'completed') {
    return 'bg-primary/15 text-primary'
  }
  if (status === 'failed') {
    return 'bg-destructive/10 text-destructive'
  }
  if (status === 'cancelled') {
    return 'bg-muted text-muted-foreground'
  }
  return 'bg-background text-foreground'
}

export function errorMessageFromUnknown(error: unknown): string {
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

export function SummaryItem({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className='rounded-xl bg-background/80 p-3'>
      <p className='text-xs uppercase tracking-[0.16em] text-muted-foreground'>{label}</p>
      <p className='mt-2 text-xl font-semibold tracking-tight'>{value}</p>
      {hint ? <p className='mt-1 text-xs text-muted-foreground'>{hint}</p> : null}
    </div>
  )
}

export function PaginationBar({
  loading,
  page,
  total,
  hasMore,
  onPrev,
  onNext,
}: {
  loading: boolean
  page: number
  total: number
  hasMore: boolean
  onPrev: () => void
  onNext: () => void
}) {
  return (
    <div className='flex items-center justify-end gap-2 pt-3'>
      <p className='mr-auto text-xs text-muted-foreground'>共 {formatNumber(total)} 条</p>
      <Button
        disabled={loading || page <= 1}
        onClick={onPrev}
        size='sm'
        type='button'
        variant='outline'
      >
        上一页
      </Button>
      <span className='text-xs text-muted-foreground'>第 {page} 页</span>
      <Button
        disabled={loading || !hasMore}
        onClick={onNext}
        size='sm'
        type='button'
        variant='outline'
      >
        下一页
      </Button>
    </div>
  )
}

export function RangeSegment({
  range,
  onChange,
}: {
  range: UsageStatsRange
  onChange: (range: UsageStatsRange) => void
}) {
  return (
    <div className='flex items-center gap-2 rounded-xl bg-background/80 p-1'>
      {rangeOptions.map((item) => (
        <Button
          className={cn('h-8 rounded-lg px-3', range === item.value && 'shadow-none')}
          key={item.value}
          onClick={() => onChange(item.value)}
          size='sm'
          type='button'
          variant={range === item.value ? 'default' : 'ghost'}
        >
          {item.label}
        </Button>
      ))}
    </div>
  )
}
