import { Loader2 } from 'lucide-react'

import type { UsageModelStatsPayload } from '@/lib/types'
import { SETTINGS_PANEL_CLASSNAME } from '@/pages/settings/lib/ui'

import { PaginationBar, formatDuration, formatNumber } from './usage-shared'

export function UsageModelsTab({
  loading,
  data,
  page,
  onPrevPage,
  onNextPage,
}: {
  loading: boolean
  data: UsageModelStatsPayload | null
  page: number
  onPrevPage: () => void
  onNextPage: () => void
}) {
  return (
    <div className='space-y-4 pt-3'>
      <div className='space-y-2'>
        {loading ? (
          <div className='flex items-center gap-2 rounded-xl px-3 py-5 text-sm text-muted-foreground'>
            <Loader2 className='h-4 w-4 animate-spin' /> 加载模型统计中...
          </div>
        ) : data?.items.length ? (
          data.items.map((item, index) => (
            <div
              className='grid gap-2 rounded-xl bg-background/75 p-3 md:grid-cols-[32px_minmax(0,1fr)_repeat(4,minmax(0,1fr))] md:items-center'
              key={`${item.model_id ?? 'unknown'}-${index}`}
            >
              <p className='text-sm font-semibold text-muted-foreground'>#{index + 1}</p>
              <div className='min-w-0'>
                <p className='truncate text-sm font-medium'>
                  {item.model_name ?? item.model ?? '未识别模型'}
                </p>
                <p className='truncate text-xs text-muted-foreground'>
                  {item.provider_name ?? '未识别服务商'}
                </p>
              </div>
              <p className='text-xs text-muted-foreground'>Turn {formatNumber(item.turn_count)}</p>
              <p className='text-xs text-muted-foreground'>成功 {formatNumber(item.completed_count)}</p>
              <p className='text-xs text-muted-foreground'>Token {formatNumber(item.total_tokens)}</p>
              <p className='text-xs text-muted-foreground'>
                均耗时 {formatDuration(item.avg_duration_ms)}
              </p>
            </div>
          ))
        ) : (
          <div className={`${SETTINGS_PANEL_CLASSNAME} text-sm text-muted-foreground`}>
            当前范围暂无模型统计。
          </div>
        )}
      </div>

      <PaginationBar
        hasMore={data?.page.has_more ?? false}
        loading={loading}
        onNext={onNextPage}
        onPrev={onPrevPage}
        page={data?.page.page ?? page}
        total={data?.page.total ?? 0}
      />
    </div>
  )
}
