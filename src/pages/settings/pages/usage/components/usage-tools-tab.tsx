import { Loader2 } from 'lucide-react'

import type { UsageToolStatsPayload } from '@/lib/types'
import { SETTINGS_PANEL_CLASSNAME } from '@/pages/settings/lib/ui'

import { PaginationBar, formatDuration, formatNumber } from './usage-shared'

export function UsageToolsTab({
  loading,
  data,
  page,
  onPrevPage,
  onNextPage,
}: {
  loading: boolean
  data: UsageToolStatsPayload | null
  page: number
  onPrevPage: () => void
  onNextPage: () => void
}) {
  return (
    <div className='space-y-4 pt-3'>
      <div className='space-y-2'>
        {loading ? (
          <div className='flex items-center gap-2 rounded-xl px-3 py-5 text-sm text-muted-foreground'>
            <Loader2 className='h-4 w-4 animate-spin' /> 加载工具统计中...
          </div>
        ) : data?.items.length ? (
          data.items.map((item, index) => (
            <div
              className='grid gap-2 rounded-xl bg-background/75 p-3 md:grid-cols-[32px_minmax(0,1fr)_repeat(4,minmax(0,1fr))] md:items-center'
              key={`${item.tool_name}-${item.tool_action ?? 'all'}-${index}`}
            >
              <p className='text-sm font-semibold text-muted-foreground'>#{index + 1}</p>
              <div className='min-w-0'>
                <p className='truncate text-sm font-medium'>{item.tool_name}</p>
                <p className='truncate text-xs text-muted-foreground'>
                  {item.tool_action ?? '(无动作标识)'}
                </p>
              </div>
              <p className='text-xs text-muted-foreground'>调用 {formatNumber(item.call_count)}</p>
              <p className='text-xs text-muted-foreground'>
                成功 {formatNumber(item.success_count)}
              </p>
              <p className='text-xs text-muted-foreground'>错误 {formatNumber(item.error_count)}</p>
              <p className='text-xs text-muted-foreground'>
                均耗时 {formatDuration(item.avg_duration_ms)}
              </p>
            </div>
          ))
        ) : (
          <div className={`${SETTINGS_PANEL_CLASSNAME} text-sm text-muted-foreground`}>
            当前范围暂无工具统计。
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
