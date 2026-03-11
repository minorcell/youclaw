import { Loader2, RefreshCw, Save } from 'lucide-react'
import { useCallback, useEffect, useMemo, useState } from 'react'

import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
import { useToastContext } from '@/contexts/toast-context'
import { getAppClient } from '@/lib/app-client'
import type { AgentConfigPayload, WorkspaceFileInfo } from '@/lib/types'

interface WorkspaceFilesPayload {
  files: WorkspaceFileInfo[]
}

interface WorkspaceFileReadPayload {
  path: string
  content: string
}

interface MemoryReindexPayload {
  indexed_chunks: number
  files_indexed: number
}

type AgentSettingsMode = 'config' | 'files'

interface AgentSettingsSectionProps {
  mode: AgentSettingsMode
}

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

export function AgentSettingsSection({ mode }: AgentSettingsSectionProps) {
  const { success: toastSuccess, error: toastError } = useToastContext()
  const [loading, setLoading] = useState(true)

  const [config, setConfig] = useState<AgentConfigPayload | null>(null)
  const [savingConfig, setSavingConfig] = useState(false)
  const [form, setForm] = useState({
    maxSteps: '8',
    maxInputTokens: '32768',
    compactRatio: '0.7',
    keepRecent: '8',
    heartbeatEnabled: false,
    heartbeatEvery: '30m',
    heartbeatTarget: 'main',
    activeStart: '',
    activeEnd: '',
  })

  const [files, setFiles] = useState<WorkspaceFileInfo[]>([])
  const [selectedPath, setSelectedPath] = useState('')
  const [fileContent, setFileContent] = useState('')
  const [fileLoading, setFileLoading] = useState(false)
  const [savingFile, setSavingFile] = useState(false)
  const [reindexing, setReindexing] = useState(false)

  const syncForm = useCallback((next: AgentConfigPayload) => {
    setConfig(next)
    setForm({
      maxSteps: String(next.max_steps),
      maxInputTokens: String(next.max_input_tokens),
      compactRatio: String(next.compact_ratio),
      keepRecent: String(next.keep_recent),
      heartbeatEnabled: next.heartbeat.enabled,
      heartbeatEvery: next.heartbeat.every,
      heartbeatTarget: next.heartbeat.target,
      activeStart: next.heartbeat.active_hours?.start ?? '',
      activeEnd: next.heartbeat.active_hours?.end ?? '',
    })
  }, [])

  const loadConfig = useCallback(async () => {
    const payload = await getAppClient().request<AgentConfigPayload>('agent.config.get', {})
    syncForm(payload)
  }, [syncForm])

  const loadFiles = useCallback(async () => {
    const payload = await getAppClient().request<WorkspaceFilesPayload>(
      'agent.workspace.files.list',
      {},
    )
    const nextFiles = payload.files ?? []
    setFiles(nextFiles)
    setSelectedPath((current) => {
      if (current && nextFiles.some((item) => item.path === current)) {
        return current
      }
      return nextFiles[0]?.path ?? ''
    })
  }, [])

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    const task = mode === 'config' ? loadConfig() : loadFiles()
    void task
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
  }, [loadConfig, loadFiles, mode, toastError])

  useEffect(() => {
    if (mode !== 'files') {
      return
    }
    if (!selectedPath) {
      setFileContent('')
      return
    }
    let cancelled = false
    setFileLoading(true)
    void getAppClient()
      .request<WorkspaceFileReadPayload>('agent.workspace.files.read', {
        path: selectedPath,
      })
      .then((payload) => {
        if (!cancelled) {
          setFileContent(payload.content ?? '')
        }
      })
      .catch((error) => {
        if (!cancelled) {
          toastError(errorText(error))
        }
      })
      .finally(() => {
        if (!cancelled) {
          setFileLoading(false)
        }
      })
    return () => {
      cancelled = true
    }
  }, [mode, selectedPath, toastError])

  const selectedFileSize = useMemo(() => {
    return files.find((item) => item.path === selectedPath)?.size ?? 0
  }, [files, selectedPath])

  async function handleSaveConfig() {
    if (!config) return
    setSavingConfig(true)
    try {
      const payload = await getAppClient().request<AgentConfigPayload>('agent.config.update', {
        max_steps: clampNumber(form.maxSteps, config.max_steps, 1, 32),
        max_input_tokens: clampNumber(
          form.maxInputTokens,
          config.max_input_tokens,
          1000,
          1_000_000,
        ),
        compact_ratio: clampNumber(form.compactRatio, config.compact_ratio, 0.1, 0.95),
        keep_recent: clampNumber(form.keepRecent, config.keep_recent, 1, 128),
        heartbeat: {
          enabled: form.heartbeatEnabled,
          every: form.heartbeatEvery.trim() || '30m',
          target: form.heartbeatTarget.trim() || 'main',
          active_hours:
            form.activeStart.trim() && form.activeEnd.trim()
              ? {
                  start: form.activeStart.trim(),
                  end: form.activeEnd.trim(),
                }
              : null,
        },
      })
      syncForm(payload)
      toastSuccess('Agent 配置已保存。')
    } catch (error) {
      toastError(errorText(error))
    } finally {
      setSavingConfig(false)
    }
  }

  async function handleSaveFile() {
    if (!selectedPath) return
    setSavingFile(true)
    try {
      await getAppClient().request('agent.workspace.files.write', {
        path: selectedPath,
        content: fileContent,
      })
      await loadFiles()
      toastSuccess('记忆文件已保存。')
    } catch (error) {
      toastError(errorText(error))
    } finally {
      setSavingFile(false)
    }
  }

  async function handleReindex() {
    setReindexing(true)
    try {
      const payload = await getAppClient().request<MemoryReindexPayload>('agent.memory.reindex', {})
      toastSuccess(
        `记忆索引已更新：${payload.files_indexed} 文件 / ${payload.indexed_chunks} 分片。`,
      )
    } catch (error) {
      toastError(errorText(error))
    } finally {
      setReindexing(false)
    }
  }

  if (loading) {
    return (
      <div className='rounded-xl border border-border/70 bg-background/75 px-4 py-6 text-sm text-muted-foreground'>
        <Loader2 className='mr-2 inline h-4 w-4 animate-spin' />
        {mode === 'config' ? '正在加载 Agent 配置...' : '正在加载记忆文件...'}
      </div>
    )
  }

  if (mode === 'config') {
    return (
      <Card className='border border-border/70 bg-background/80'>
        <CardHeader>
          <CardTitle>Agent 配置</CardTitle>
          <CardDescription>调整 steps、上下文压缩阈值和 heartbeat 定时执行参数。</CardDescription>
        </CardHeader>
        <CardContent className='space-y-4'>
          <div className='grid grid-cols-2 gap-3'>
            <div className='space-y-1.5'>
              <Label htmlFor='agent-max-steps'>max_steps</Label>
              <Input
                id='agent-max-steps'
                onChange={(event) => setForm((prev) => ({ ...prev, maxSteps: event.target.value }))}
                value={form.maxSteps}
              />
            </div>
            <div className='space-y-1.5'>
              <Label htmlFor='agent-max-input'>max_input_tokens</Label>
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
            <div className='space-y-1.5'>
              <Label htmlFor='agent-compact-ratio'>compact_ratio</Label>
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
            <div className='space-y-1.5'>
              <Label htmlFor='agent-keep-recent'>keep_recent</Label>
              <Input
                id='agent-keep-recent'
                onChange={(event) =>
                  setForm((prev) => ({ ...prev, keepRecent: event.target.value }))
                }
                value={form.keepRecent}
              />
            </div>
          </div>

          <div className='grid grid-cols-2 gap-3'>
            <div className='col-span-2 flex items-center justify-between rounded-xl border border-border/70 bg-card/60 px-3 py-2'>
              <div>
                <p className='text-sm font-medium'>heartbeat.enabled</p>
                <p className='text-xs text-muted-foreground'>开启后台周期执行</p>
              </div>
              <Switch
                checked={form.heartbeatEnabled}
                onCheckedChange={(checked) =>
                  setForm((prev) => ({ ...prev, heartbeatEnabled: checked }))
                }
              />
            </div>
            <div className='space-y-1.5'>
              <Label htmlFor='agent-heartbeat-every'>heartbeat.every</Label>
              <Input
                id='agent-heartbeat-every'
                onChange={(event) =>
                  setForm((prev) => ({
                    ...prev,
                    heartbeatEvery: event.target.value,
                  }))
                }
                placeholder='30m'
                value={form.heartbeatEvery}
              />
            </div>
            <div className='space-y-1.5'>
              <Label htmlFor='agent-heartbeat-target'>heartbeat.target</Label>
              <Input
                id='agent-heartbeat-target'
                onChange={(event) =>
                  setForm((prev) => ({
                    ...prev,
                    heartbeatTarget: event.target.value,
                  }))
                }
                placeholder='main'
                value={form.heartbeatTarget}
              />
            </div>
            <div className='space-y-1.5'>
              <Label htmlFor='agent-active-start'>active start (HH:MM)</Label>
              <Input
                id='agent-active-start'
                onChange={(event) =>
                  setForm((prev) => ({ ...prev, activeStart: event.target.value }))
                }
                placeholder='08:00'
                value={form.activeStart}
              />
            </div>
            <div className='space-y-1.5'>
              <Label htmlFor='agent-active-end'>active end (HH:MM)</Label>
              <Input
                id='agent-active-end'
                onChange={(event) =>
                  setForm((prev) => ({ ...prev, activeEnd: event.target.value }))
                }
                placeholder='22:00'
                value={form.activeEnd}
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
              保存 Agent 配置
            </Button>
          </div>
        </CardContent>
      </Card>
    )
  }

  return (
    <Card className='border border-border/70 bg-background/80'>
      <CardHeader>
        <CardTitle>记忆文件</CardTitle>
        <CardDescription>
          直接编辑 `PROFILE.md / MEMORY.md / memory/*.md / HEARTBEAT.md` 等工作区文件。
        </CardDescription>
      </CardHeader>
      <CardContent className='space-y-3'>
        <div className='space-y-1.5'>
          <Label htmlFor='memory-file-select'>文件</Label>
          <select
            className='h-9 w-full rounded-md border border-input bg-background px-3 text-sm'
            id='memory-file-select'
            onChange={(event) => setSelectedPath(event.target.value)}
            value={selectedPath}
          >
            {files.length === 0 ? <option value=''>无文件</option> : null}
            {files.map((item) => (
              <option key={item.path} value={item.path}>
                {item.path}
              </option>
            ))}
          </select>
          <p className='text-xs text-muted-foreground'>当前文件大小：{selectedFileSize} bytes</p>
        </div>

        <div className='space-y-1.5'>
          <Label htmlFor='memory-file-content'>内容</Label>
          <Textarea
            className='min-h-[420px] font-mono text-xs leading-5'
            id='memory-file-content'
            onChange={(event) => setFileContent(event.target.value)}
            placeholder={fileLoading ? '读取中...' : '选择文件后可编辑'}
            value={fileContent}
          />
        </div>

        <div className='flex flex-wrap gap-2'>
          <Button
            disabled={!selectedPath || fileLoading || savingFile}
            onClick={() => void handleSaveFile()}
            size='sm'
            type='button'
          >
            {savingFile ? (
              <Loader2 className='mr-1 h-4 w-4 animate-spin' />
            ) : (
              <Save className='mr-1 h-4 w-4' />
            )}
            保存文件
          </Button>
          <Button
            disabled={reindexing}
            onClick={() => void handleReindex()}
            size='sm'
            type='button'
            variant='outline'
          >
            {reindexing ? (
              <Loader2 className='mr-1 h-4 w-4 animate-spin' />
            ) : (
              <RefreshCw className='mr-1 h-4 w-4' />
            )}
            重建记忆索引
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
