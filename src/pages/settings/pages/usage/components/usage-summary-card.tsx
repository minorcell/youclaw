import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import type { UsageStatsRange, UsageSummaryPayload } from '@/lib/types'
import { SETTINGS_CARD_CLASSNAME, SETTINGS_CARD_HEADER_CLASSNAME } from '@/pages/settings/lib/ui'

import { RangeSegment, SummaryItem, formatNumber } from './usage-shared'

export function UsageSummaryCard({
  range,
  onRangeChange,
  summary,
  summaryLoading,
}: {
  range: UsageStatsRange
  onRangeChange: (range: UsageStatsRange) => void
  summary: UsageSummaryPayload | null
  summaryLoading: boolean
}) {
  return (
    <Card className={SETTINGS_CARD_CLASSNAME}>
      <CardHeader className={`${SETTINGS_CARD_HEADER_CLASSNAME} space-y-4`}>
        <div className='flex flex-wrap items-center justify-between gap-3'>
          <div>
            <CardTitle>使用概览</CardTitle>
            <CardDescription>按时间范围查看 Turn 与 Token 消耗。</CardDescription>
          </div>
          <RangeSegment onChange={onRangeChange} range={range} />
        </div>
      </CardHeader>

      <CardContent className='grid gap-3 py-4 sm:grid-cols-2 xl:grid-cols-4'>
        <SummaryItem
          hint={`总 Step ${formatNumber(summary?.total_steps ?? 0)}`}
          label='总 Turn'
          value={summaryLoading ? '...' : formatNumber(summary?.total_turns ?? 0)}
        />
        <SummaryItem
          hint={`缓存读取 ${formatNumber(summary?.input_cache_read_tokens ?? 0)}`}
          label='输入 Token'
          value={summaryLoading ? '...' : formatNumber(summary?.input_tokens ?? 0)}
        />
        <SummaryItem
          hint={`推理 ${formatNumber(summary?.reasoning_tokens ?? 0)}`}
          label='输出 Token'
          value={summaryLoading ? '...' : formatNumber(summary?.output_tokens ?? 0)}
        />
        <SummaryItem
          hint={`平均步数 ${((summary?.avg_steps_per_turn ?? 0) || 0).toFixed(2)}`}
          label='总 Token'
          value={summaryLoading ? '...' : formatNumber(summary?.total_tokens ?? 0)}
        />
      </CardContent>
    </Card>
  )
}
