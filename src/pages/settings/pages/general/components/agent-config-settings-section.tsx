import { Loader2, Save } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'

import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useToastContext } from '@/contexts/toast-context'
import { getAppClient } from '@/lib/app-client'
import type { AgentConfigPayload } from '@/lib/types'
import {
  SETTINGS_CARD_CLASSNAME,
  SETTINGS_CARD_CONTENT_CLASSNAME,
  SETTINGS_CARD_HEADER_CLASSNAME,
  SETTINGS_PANEL_CLASSNAME,
} from '@/pages/settings/lib/ui'

const MIN_MAX_STEPS = 8
const MAX_MAX_STEPS = 128
const DEFAULT_MAX_STEPS = 64

function errorText(error: unknown): string {
  if (error instanceof Error) return error.message
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

function clampNumber(value: string, fallback: number, min: number, max: number): number {
  const parsed = Number(value)
  if (!Number.isFinite(parsed)) return fallback
  return Math.min(max, Math.max(min, parsed))
}

export function AgentConfigSettingsSection() {
  const { success: toastSuccess, error: toastError } = useToastContext()
  const [loading, setLoading] = useState(true)
  const [config, setConfig] = useState<AgentConfigPayload | null>(null)
  const [savingConfig, setSavingConfig] = useState(false)
  const [form, setForm] = useState({
    maxSteps: String(DEFAULT_MAX_STEPS),
    maxInputTokens: '120000',
    compactRatio: '0.8',
  })

  const syncForm = useCallback((next: AgentConfigPayload) => {
    setConfig(next)
    setForm({
      maxSteps: String(next.max_steps),
      maxInputTokens: String(next.max_input_tokens),
      compactRatio: String(next.compact_ratio),
    })
  }, [])

  const loadConfig = useCallback(async () => {
    const payload = await getAppClient().request<AgentConfigPayload>('agent.config.get', {})
    syncForm(payload)
  }, [syncForm])

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    void loadConfig()
      .catch((error) => {
        if (!cancelled) {
          toastError(errorText(error))
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false)
        }
      })
    return () => {
      cancelled = true
    }
  }, [loadConfig, toastError])

  async function handleSaveConfig() {
    if (!config) return
    setSavingConfig(true)
    try {
      const payload = await getAppClient().request<AgentConfigPayload>('agent.config.update', {
        max_steps: clampNumber(form.maxSteps, config.max_steps, MIN_MAX_STEPS, MAX_MAX_STEPS),
        max_input_tokens: clampNumber(
          form.maxInputTokens,
          config.max_input_tokens,
          75_000,
          200_000,
        ),
        compact_ratio: clampNumber(form.compactRatio, config.compact_ratio, 0.1, 0.95),
      })
      syncForm(payload)
      toastSuccess('智能体配置已保存。')
    } catch (error) {
      toastError(errorText(error))
    } finally {
      setSavingConfig(false)
    }
  }

  if (loading) {
    return (
      <div className='rounded-2xl bg-background/80 px-4 py-6 text-sm text-muted-foreground'>
        <Loader2 className='mr-2 inline h-4 w-4 animate-spin' />
        正在加载智能体配置...
      </div>
    )
  }

  return (
    <Card className={SETTINGS_CARD_CLASSNAME}>
      <CardHeader className={SETTINGS_CARD_HEADER_CLASSNAME}>
        <CardTitle>智能体配置</CardTitle>
        <CardDescription>调整执行步数、默认上下文窗口与压缩策略。</CardDescription>
      </CardHeader>
      <CardContent className={SETTINGS_CARD_CONTENT_CLASSNAME}>
        <div className='grid gap-3 xl:grid-cols-2'>
          <div className={`${SETTINGS_PANEL_CLASSNAME} space-y-1.5`}>
            <Label htmlFor='agent-max-steps'>最大执行步数</Label>
            <Input
              id='agent-max-steps'
              onChange={(event) => setForm((prev) => ({ ...prev, maxSteps: event.target.value }))}
              value={form.maxSteps}
            />
            <p className='text-xs text-muted-foreground'>
              范围 {MIN_MAX_STEPS} - {MAX_MAX_STEPS}，默认 {DEFAULT_MAX_STEPS}。
            </p>
          </div>
          <div className={`${SETTINGS_PANEL_CLASSNAME} space-y-1.5`}>
            <Label htmlFor='agent-max-input'>默认上下文窗口长度（Token）</Label>
            <Input
              id='agent-max-input'
              onChange={(event) =>
                setForm((prev) => ({
                  ...prev,
                  maxInputTokens: event.target.value,
                }))
              }
              value={form.maxInputTokens}
            />
          </div>
          <div className={`${SETTINGS_PANEL_CLASSNAME} space-y-1.5 xl:col-span-2`}>
            <Label htmlFor='agent-compact-ratio'>上下文压缩比例</Label>
            <Input
              id='agent-compact-ratio'
              onChange={(event) =>
                setForm((prev) => ({
                  ...prev,
                  compactRatio: event.target.value,
                }))
              }
              value={form.compactRatio}
            />
          </div>
        </div>

        <div className='flex flex-wrap gap-2'>
          <Button
            disabled={savingConfig}
            onClick={() => void handleSaveConfig()}
            size='sm'
            type='button'
          >
            {savingConfig ? (
              <Loader2 className='mr-1 h-4 w-4 animate-spin' />
            ) : (
              <Save className='mr-1 h-4 w-4' />
            )}
            保存智能体配置
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
