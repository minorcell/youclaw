import { Loader2, Plus, Save, Trash2 } from 'lucide-react'
import { useCallback, useEffect, useMemo, useState } from 'react'

import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Textarea } from '@/components/ui/textarea'
import { useToastContext } from '@/contexts/toast-context'
import { getAppClient } from '@/lib/app-client'
import type {
  AgentProfile,
  MemoryRecord,
  MemoryRecordSummary,
  ProfileTarget,
} from '@/lib/types'
import {
  SETTINGS_CARD_CLASSNAME,
  SETTINGS_CARD_CONTENT_CLASSNAME,
  SETTINGS_CARD_HEADER_CLASSNAME,
  SETTINGS_PANEL_CLASSNAME,
} from '@/pages/settings/lib/ui'

const NEW_MEMORY_ID = '__new__'
const PROFILE_TARGET_ORDER: ProfileTarget[] = ['user', 'soul']
const PROFILE_TARGET_META: Record<ProfileTarget, { label: string; description: string }> = {
  user: {
    label: 'User Profile',
    description: '用户身份、稳定偏好、沟通方式与长期协作约束。',
  },
  soul: {
    label: 'Agent Soul',
    description: 'Agent 自身的长期协作方式、推进原则与风险处理风格。',
  },
}

interface MemorySystemListPayload {
  entries: MemoryRecordSummary[]
}

interface MemorySystemGetPayload {
  entry: MemoryRecord
}

interface MemorySystemWritePayload {
  entry: MemoryRecord
  created: boolean
}

interface ProfileGetPayload {
  profiles: AgentProfile[]
  needs_onboarding: boolean
  missing_targets: ProfileTarget[]
}

interface ProfileWritePayload {
  profile: AgentProfile
  needs_onboarding: boolean
  missing_targets: ProfileTarget[]
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

function emptyProfileMap(): Record<ProfileTarget, AgentProfile | null> {
  return {
    user: null,
    soul: null,
  }
}

function emptyProfileDrafts(): Record<ProfileTarget, string> {
  return {
    user: '',
    soul: '',
  }
}

export function AgentMemoryFilesSection() {
  const { success: toastSuccess, error: toastError } = useToastContext()
  const [memoryLoading, setMemoryLoading] = useState(true)
  const [entries, setEntries] = useState<MemoryRecordSummary[]>([])
  const [selectedId, setSelectedId] = useState(NEW_MEMORY_ID)
  const [title, setTitle] = useState('')
  const [content, setContent] = useState('')
  const [entryLoading, setEntryLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const [deleting, setDeleting] = useState(false)

  const [profilesLoading, setProfilesLoading] = useState(true)
  const [profiles, setProfiles] = useState<Record<ProfileTarget, AgentProfile | null>>(emptyProfileMap)
  const [profileDrafts, setProfileDrafts] =
    useState<Record<ProfileTarget, string>>(emptyProfileDrafts)
  const [profileSavingTarget, setProfileSavingTarget] = useState<ProfileTarget | null>(null)
  const [missingProfileTargets, setMissingProfileTargets] = useState<ProfileTarget[]>([])

  const loadEntries = useCallback(async () => {
    const payload = await getAppClient().request<MemorySystemListPayload>('agent.memory_system.list', {
      limit: 100,
    })
    const nextEntries = payload.entries ?? []
    setEntries(nextEntries)
    setSelectedId((current) => {
      if (current === NEW_MEMORY_ID) {
        return current
      }
      if (current && nextEntries.some((entry) => entry.id === current)) {
        return current
      }
      return nextEntries[0]?.id ?? NEW_MEMORY_ID
    })
  }, [])

  const loadProfiles = useCallback(async () => {
    const payload = await getAppClient().request<ProfileGetPayload>('agent.profile.get', {})
    const nextProfiles = emptyProfileMap()
    for (const profile of payload.profiles ?? []) {
      nextProfiles[profile.target] = profile
    }
    setProfiles(nextProfiles)
    setProfileDrafts({
      user: nextProfiles.user?.content ?? '',
      soul: nextProfiles.soul?.content ?? '',
    })
    setMissingProfileTargets(payload.missing_targets ?? [])
  }, [])

  useEffect(() => {
    let cancelled = false
    setMemoryLoading(true)
    void loadEntries()
      .catch((error) => {
        if (!cancelled) {
          toastError(errorText(error))
        }
      })
      .finally(() => {
        if (!cancelled) {
          setMemoryLoading(false)
        }
      })
    return () => {
      cancelled = true
    }
  }, [loadEntries, toastError])

  useEffect(() => {
    let cancelled = false
    setProfilesLoading(true)
    void loadProfiles()
      .catch((error) => {
        if (!cancelled) {
          toastError(errorText(error))
        }
      })
      .finally(() => {
        if (!cancelled) {
          setProfilesLoading(false)
        }
      })
    return () => {
      cancelled = true
    }
  }, [loadProfiles, toastError])

  useEffect(() => {
    if (selectedId === NEW_MEMORY_ID) {
      setTitle('')
      setContent('')
      return
    }
    let cancelled = false
    setEntryLoading(true)
    void getAppClient()
      .request<MemorySystemGetPayload>('agent.memory_system.get', { id: selectedId })
      .then((payload) => {
        if (!cancelled) {
          setTitle(payload.entry.title ?? '')
          setContent(payload.entry.content ?? '')
        }
      })
      .catch((error) => {
        if (!cancelled) {
          toastError(errorText(error))
        }
      })
      .finally(() => {
        if (!cancelled) {
          setEntryLoading(false)
        }
      })
    return () => {
      cancelled = true
    }
  }, [selectedId, toastError])

  const selectedEntrySummary = useMemo(
    () => entries.find((entry) => entry.id === selectedId) ?? null,
    [entries, selectedId],
  )

  const canSave = !saving && title.trim().length > 0 && content.trim().length > 0
  const canDelete = selectedId !== NEW_MEMORY_ID && !deleting && !entryLoading

  async function handleSave() {
    if (!canSave) return
    setSaving(true)
    try {
      const payload = await getAppClient().request<MemorySystemWritePayload>('agent.memory_system.upsert', {
        id: selectedId === NEW_MEMORY_ID ? null : selectedId,
        title,
        content,
      })
      await loadEntries()
      setSelectedId(payload.entry.id)
      toastSuccess(payload.created ? '记忆已创建。' : '记忆已更新。')
    } catch (error) {
      toastError(errorText(error))
    } finally {
      setSaving(false)
    }
  }

  async function handleDelete() {
    if (!canDelete) return
    setDeleting(true)
    try {
      await getAppClient().request('agent.memory_system.delete', { id: selectedId })
      await loadEntries()
      setSelectedId(entries.filter((entry) => entry.id !== selectedId)[0]?.id ?? NEW_MEMORY_ID)
      toastSuccess('记忆已删除。')
    } catch (error) {
      toastError(errorText(error))
    } finally {
      setDeleting(false)
    }
  }

  async function handleSaveProfile(target: ProfileTarget) {
    const nextContent = profileDrafts[target].trim()
    if (!nextContent || profileSavingTarget) return
    setProfileSavingTarget(target)
    try {
      const payload = await getAppClient().request<ProfileWritePayload>('agent.profile.update', {
        target,
        content: nextContent,
      })
      setProfiles((current) => ({
        ...current,
        [target]: payload.profile,
      }))
      setProfileDrafts((current) => ({
        ...current,
        [target]: payload.profile.content,
      }))
      setMissingProfileTargets(payload.missing_targets ?? [])
      toastSuccess(`${PROFILE_TARGET_META[target].label} 已更新。`)
    } catch (error) {
      toastError(errorText(error))
    } finally {
      setProfileSavingTarget(null)
    }
  }

  function handleCreateNew() {
    setSelectedId(NEW_MEMORY_ID)
    setTitle('')
    setContent('')
  }

  return (
    <div className='space-y-4'>
      <Card className={SETTINGS_CARD_CLASSNAME}>
        <CardHeader className={SETTINGS_CARD_HEADER_CLASSNAME}>
          <CardTitle>Profile 系统</CardTitle>
          <CardDescription>
            `user` 与 `soul` 画像会在每轮对话中直接注入上下文，和按需检索的长期记忆分开管理。
          </CardDescription>
        </CardHeader>
        <CardContent className={SETTINGS_CARD_CONTENT_CLASSNAME}>
          {profilesLoading ? (
            <div className='rounded-2xl bg-background/80 px-4 py-6 text-sm text-muted-foreground'>
              <Loader2 className='mr-2 inline h-4 w-4 animate-spin' />
              正在加载 profile...
            </div>
          ) : (
            <>
              <div className={`${SETTINGS_PANEL_CLASSNAME} space-y-1.5`}>
                <div className='flex items-center justify-between gap-3'>
                  <Label>初始化状态</Label>
                  <span className='text-xs text-muted-foreground'>
                    {missingProfileTargets.length === 0
                      ? '已完成'
                      : `仍缺少：${missingProfileTargets.join(', ')}`}
                  </span>
                </div>
                <p className='text-xs text-muted-foreground'>
                  Agent 首次会话只会在缺失 profile 时引导用户配置；一旦 `user` 和 `soul` 都有内容，新会话不再重复引导。
                </p>
              </div>

              {PROFILE_TARGET_ORDER.map((target) => {
                const meta = PROFILE_TARGET_META[target]
                const profile = profiles[target]
                const isSaving = profileSavingTarget === target
                return (
                  <div className={`${SETTINGS_PANEL_CLASSNAME} space-y-3`} key={target}>
                    <div className='flex items-center justify-between gap-3'>
                      <div className='space-y-1'>
                        <Label htmlFor={`profile-${target}`}>{meta.label}</Label>
                        <p className='text-xs text-muted-foreground'>{meta.description}</p>
                      </div>
                      <span className='truncate text-xs text-muted-foreground'>
                        {profile?.updated_at ?? '尚未配置'}
                      </span>
                    </div>
                    <Textarea
                      className='min-h-42 rounded-lg border-border/70 bg-card/80 font-mono text-xs leading-5 shadow-none'
                      id={`profile-${target}`}
                      onChange={(event) =>
                        setProfileDrafts((current) => ({
                          ...current,
                          [target]: event.target.value,
                        }))
                      }
                      placeholder={target === 'user' ? '输入用户画像' : '输入 agent soul'}
                      value={profileDrafts[target]}
                    />
                    <div className='flex justify-end'>
                      <Button
                        disabled={isSaving || profileDrafts[target].trim().length === 0}
                        onClick={() => void handleSaveProfile(target)}
                        size='sm'
                        type='button'
                      >
                        {isSaving ? (
                          <Loader2 className='mr-1 h-4 w-4 animate-spin' />
                        ) : (
                          <Save className='mr-1 h-4 w-4' />
                        )}
                        保存 {meta.label}
                      </Button>
                    </div>
                  </div>
                )
              })}
            </>
          )}
        </CardContent>
      </Card>

      <Card className={SETTINGS_CARD_CLASSNAME}>
        <CardHeader className={SETTINGS_CARD_HEADER_CLASSNAME}>
          <CardTitle>长期记忆</CardTitle>
          <CardDescription>
            直接管理数据库中的长期记忆条目。Agent 侧只通过 `memory_system_*` 能力读写这些数据。
          </CardDescription>
        </CardHeader>
        <CardContent className={SETTINGS_CARD_CONTENT_CLASSNAME}>
          {memoryLoading ? (
            <div className='rounded-2xl bg-background/80 px-4 py-6 text-sm text-muted-foreground'>
              <Loader2 className='mr-2 inline h-4 w-4 animate-spin' />
              正在加载记忆系统...
            </div>
          ) : (
            <>
              <div className={`${SETTINGS_PANEL_CLASSNAME} space-y-1.5`}>
                <div className='flex items-center justify-between gap-3'>
                  <Label htmlFor='memory-entry-select'>条目</Label>
                  <Button onClick={handleCreateNew} size='sm' type='button' variant='outline'>
                    <Plus className='mr-1 h-4 w-4' />
                    新建记忆
                  </Button>
                </div>
                {entries.length === 0 ? (
                  <div className='rounded-lg bg-card/80 px-3 py-2 text-sm text-muted-foreground'>
                    暂无长期记忆
                  </div>
                ) : (
                  <Select onValueChange={(value) => setSelectedId(value ?? NEW_MEMORY_ID)} value={selectedId}>
                    <SelectTrigger className='h-9 w-full rounded-lg bg-card/80 shadow-none'>
                      <SelectValue placeholder='选择记忆条目' />
                    </SelectTrigger>
                    <SelectContent>
                      {entries.map((entry) => (
                        <SelectItem key={entry.id} value={entry.id}>
                          {entry.title}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                )}
                {selectedEntrySummary ? (
                  <p className='truncate text-xs text-muted-foreground'>{selectedEntrySummary.preview}</p>
                ) : (
                  <p className='text-xs text-muted-foreground'>当前正在创建新条目。</p>
                )}
              </div>

              <div className={`${SETTINGS_PANEL_CLASSNAME} space-y-3`}>
                <div className='space-y-1.5'>
                  <Label htmlFor='memory-title'>标题</Label>
                  <Input
                    id='memory-title'
                    onChange={(event) => setTitle(event.target.value)}
                    placeholder='例如：项目背景 / 长期约束 / 关键事实'
                    value={title}
                  />
                </div>

                <div className='space-y-1.5'>
                  <div className='flex items-center justify-between gap-3'>
                    <Label htmlFor='memory-content'>内容</Label>
                    {entryLoading ? (
                      <span className='text-xs text-muted-foreground'>读取中...</span>
                    ) : selectedEntrySummary ? (
                      <span className='truncate text-xs text-muted-foreground'>
                        {selectedEntrySummary.updated_at}
                      </span>
                    ) : null}
                  </div>
                  <Textarea
                    className='min-h-105 rounded-lg border-border/70 bg-card/80 font-mono text-xs leading-5 shadow-none'
                    id='memory-content'
                    onChange={(event) => setContent(event.target.value)}
                    placeholder='输入长期记忆内容'
                    value={content}
                  />
                </div>
              </div>

              <div className='flex flex-wrap gap-2'>
                <Button disabled={!canSave || entryLoading} onClick={() => void handleSave()} size='sm'>
                  {saving ? (
                    <Loader2 className='mr-1 h-4 w-4 animate-spin' />
                  ) : (
                    <Save className='mr-1 h-4 w-4' />
                  )}
                  保存记忆
                </Button>
                <Button
                  disabled={!canDelete}
                  onClick={() => void handleDelete()}
                  size='sm'
                  type='button'
                  variant='outline'
                >
                  {deleting ? (
                    <Loader2 className='mr-1 h-4 w-4 animate-spin' />
                  ) : (
                    <Trash2 className='mr-1 h-4 w-4' />
                  )}
                  删除条目
                </Button>
              </div>
            </>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
