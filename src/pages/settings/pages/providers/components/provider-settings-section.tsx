import {
  CheckCircle2,
  FlaskConical,
  Loader2,
  PencilLine,
  Plus,
  Save,
  SlidersHorizontal,
  Server,
  Trash2,
} from 'lucide-react'
import { useEffect, useMemo, useState, type FormEvent } from 'react'

import { ProviderForm } from '@/pages/settings/pages/providers/components/provider-form'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import {
  Popover,
  PopoverContent,
  PopoverDescription,
  PopoverHeader,
  PopoverTitle,
  PopoverTrigger,
} from '@/components/ui/popover'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { DEFAULT_PROVIDER_MODEL } from '@/lib/provider-defaults'
import type { ProviderAccount, ProviderModel } from '@/lib/types'
import { cn } from '@/lib/utils'

const MIN_CONTEXT_WINDOW_TOKENS = 75_000
const MAX_CONTEXT_WINDOW_TOKENS = 200_000

interface ModelInputValue {
  model: string
  contextWindowTokens: string
}

interface NormalizedModelValue {
  model: string
  context_window_tokens: number | null
  contextWindowTokensError: string | null
}

interface ProviderSettingsSectionProps {
  providers: ProviderAccount[]
  selectedProvider: ProviderAccount | null
  selectedProviderId: string | 'new'
  setSelectedProviderId: (id: string | 'new') => void
  onNewProvider: () => void
  onSaveProvider: (value: {
    profile_name: string
    base_url: string
    api_key: string
    initial_model?: string
    initial_context_window_tokens?: number | null
  }) => Promise<void>
  onCreateModel: (value: { model: string; context_window_tokens: number | null }) => Promise<void>
  onUpdateModel: (
    modelId: string,
    value: { model: string; context_window_tokens: number | null },
  ) => Promise<void>
  onDeleteModel: (modelId: string) => Promise<void>
  onTestModel: (value: { provider_id: string; model: string; model_id?: string }) => Promise<void>
  accountBusy: boolean
  modelBusyId: string | null
}

function formatProviderHost(baseUrl: string): string {
  try {
    const url = new URL(baseUrl)
    return url.host
  } catch {
    return baseUrl
  }
}

function normalizeModelValue(value: ModelInputValue): NormalizedModelValue {
  const model = value.model.trim()
  const rawContextWindow = value.contextWindowTokens.trim()
  if (rawContextWindow.length === 0) {
    return {
      model,
      context_window_tokens: null,
      contextWindowTokensError: null,
    }
  }
  const parsed = Number(rawContextWindow)
  if (!Number.isFinite(parsed)) {
    return {
      model,
      context_window_tokens: null,
      contextWindowTokensError: '上下文窗口必须是数字（留空表示使用全局默认）。',
    }
  }
  const rounded = Math.round(parsed)
  if (rounded < MIN_CONTEXT_WINDOW_TOKENS || rounded > MAX_CONTEXT_WINDOW_TOKENS) {
    return {
      model,
      context_window_tokens: null,
      contextWindowTokensError: `上下文窗口需在 ${MIN_CONTEXT_WINDOW_TOKENS} 到 ${MAX_CONTEXT_WINDOW_TOKENS} 之间。`,
    }
  }
  return {
    model,
    context_window_tokens: rounded,
    contextWindowTokensError: null,
  }
}

function sameModelValue(left: NormalizedModelValue, right: NormalizedModelValue) {
  return left.model === right.model && left.context_window_tokens === right.context_window_tokens
}

function formatContextWindowTokens(tokens: number): string {
  return tokens.toLocaleString('en-US')
}

function ContextWindowAdvancedControl({
  value,
  disabled,
  error,
  align = 'end',
  onChange,
}: {
  value: string
  disabled: boolean
  error: string | null
  align?: 'start' | 'center' | 'end'
  onChange: (value: string) => void
}) {
  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button
            disabled={disabled}
            size='sm'
            type='button'
            variant={value.trim().length > 0 ? 'secondary' : 'outline'}
          />
        }
      >
        <SlidersHorizontal className='h-3.5 w-3.5' />
        高级设置
      </PopoverTrigger>

      <PopoverContent align={align} className='w-[min(22rem,calc(100vw-2rem))]'>
        <PopoverHeader>
          <PopoverTitle>上下文窗口</PopoverTitle>
          <PopoverDescription>
            对当前模型覆盖全局默认窗口；留空则继续使用全局设置。
          </PopoverDescription>
        </PopoverHeader>

        <div className='space-y-1.5'>
          <Input
            disabled={disabled}
            inputMode='numeric'
            onChange={(event) => onChange(event.target.value)}
            placeholder={`${MIN_CONTEXT_WINDOW_TOKENS} - ${MAX_CONTEXT_WINDOW_TOKENS}`}
            value={value}
          />
          {error ? (
            <p className='text-xs text-destructive'>{error}</p>
          ) : (
            <p className='text-xs text-muted-foreground'>
              可填写 {formatContextWindowTokens(MIN_CONTEXT_WINDOW_TOKENS)} 到{' '}
              {formatContextWindowTokens(MAX_CONTEXT_WINDOW_TOKENS)}。
            </p>
          )}
        </div>
      </PopoverContent>
    </Popover>
  )
}

function ProviderModelItem({
  model,
  providerId,
  modelBusyId,
  onUpdateModel,
  onDeleteModel,
  onTestModel,
}: {
  model: ProviderModel
  providerId: string
  modelBusyId: string | null
  onUpdateModel: (
    modelId: string,
    value: { model: string; context_window_tokens: number | null },
  ) => Promise<void>
  onDeleteModel: (modelId: string) => Promise<void>
  onTestModel: (value: { provider_id: string; model: string; model_id?: string }) => Promise<void>
}) {
  const initial = useMemo<ModelInputValue>(
    () => ({
      model: model.model,
      contextWindowTokens:
        model.context_window_tokens === null || model.context_window_tokens === undefined
          ? ''
          : String(model.context_window_tokens),
    }),
    [model.context_window_tokens, model.model],
  )

  const [form, setForm] = useState(initial)

  useEffect(() => {
    setForm(initial)
  }, [initial])

  const normalizedInitial = useMemo(() => normalizeModelValue(initial), [initial])
  const normalizedCurrent = useMemo(() => normalizeModelValue(form), [form])
  const isDirty = !sameModelValue(normalizedCurrent, normalizedInitial)
  const canSubmit =
    normalizedCurrent.model.length > 0 && normalizedCurrent.contextWindowTokensError === null
  const isSaving = modelBusyId === `save:${model.id}`
  const isTesting = modelBusyId === `test:${model.id}`
  const isDeleting = modelBusyId === `delete:${model.id}`
  const isBusy = isSaving || isTesting || isDeleting

  return (
    <div className='rounded-xl bg-background/85 p-2'>
      <div className='space-y-2'>
        <div className='flex items-center gap-2 overflow-x-auto'>
          <Input
            className='min-w-[220px] flex-1'
            disabled={isBusy}
            onChange={(event) =>
              setForm((current) => ({
                ...current,
                model: event.target.value,
              }))
            }
            placeholder={DEFAULT_PROVIDER_MODEL}
            value={form.model}
          />
          <ContextWindowAdvancedControl
            disabled={isBusy}
            error={normalizedCurrent.contextWindowTokensError}
            onChange={(value) =>
              setForm((current) => ({
                ...current,
                contextWindowTokens: value,
              }))
            }
            value={form.contextWindowTokens}
          />
          {normalizedCurrent.context_window_tokens === null ? null : (
            <Badge className='shrink-0 bg-background text-foreground'>
              {formatContextWindowTokens(normalizedCurrent.context_window_tokens)} tokens
            </Badge>
          )}

          <TooltipProvider>
            <div className='flex shrink-0 items-center gap-1'>
              <Tooltip>
                <TooltipTrigger
                  render={
                    <Button
                      aria-label='保存模型配置'
                      disabled={!canSubmit || !isDirty || isBusy}
                      onClick={() => void onUpdateModel(model.id, normalizedCurrent)}
                      size='icon-sm'
                      type='button'
                    >
                      {isSaving ? (
                        <Loader2 className='h-3.5 w-3.5 animate-spin' />
                      ) : (
                        <Save className='h-3.5 w-3.5' />
                      )}
                    </Button>
                  }
                />
                <TooltipContent>保存</TooltipContent>
              </Tooltip>

              <Tooltip>
                <TooltipTrigger
                  render={
                    <Button
                      aria-label='测试模型连接'
                      disabled={!canSubmit || isBusy}
                      onClick={() =>
                        void onTestModel({
                          provider_id: providerId,
                          model: normalizedCurrent.model,
                          model_id: model.id,
                        })
                      }
                      size='icon-sm'
                      type='button'
                      variant='outline'
                    >
                      {isTesting ? (
                        <Loader2 className='h-3.5 w-3.5 animate-spin' />
                      ) : (
                        <FlaskConical className='h-3.5 w-3.5' />
                      )}
                    </Button>
                  }
                />
                <TooltipContent>测试</TooltipContent>
              </Tooltip>

              <Tooltip>
                <TooltipTrigger
                  render={
                    <Button
                      aria-label='删除模型配置'
                      className='text-destructive hover:text-destructive'
                      disabled={isBusy}
                      onClick={() => void onDeleteModel(model.id)}
                      size='icon-sm'
                      type='button'
                      variant='ghost'
                    >
                      {isDeleting ? (
                        <Loader2 className='h-3.5 w-3.5 animate-spin' />
                      ) : (
                        <Trash2 className='h-3.5 w-3.5' />
                      )}
                    </Button>
                  }
                />
                <TooltipContent>删除</TooltipContent>
              </Tooltip>
            </div>
          </TooltipProvider>
        </div>
        {normalizedCurrent.contextWindowTokensError ? (
          <p className='text-xs text-destructive'>{normalizedCurrent.contextWindowTokensError}</p>
        ) : (
          <p className='text-xs text-muted-foreground'>留空表示使用全局默认窗口。</p>
        )}
      </div>
    </div>
  )
}

export function ProviderSettingsSection({
  providers,
  selectedProvider,
  selectedProviderId,
  setSelectedProviderId,
  onNewProvider,
  onSaveProvider,
  onCreateModel,
  onUpdateModel,
  onDeleteModel,
  onTestModel,
  accountBusy,
  modelBusyId,
}: ProviderSettingsSectionProps) {
  const isCreatingNew = selectedProviderId === 'new'
  const [newModel, setNewModel] = useState(DEFAULT_PROVIDER_MODEL)
  const [newModelContextWindowTokens, setNewModelContextWindowTokens] = useState('')
  const [newModelError, setNewModelError] = useState<string | null>(null)
  const normalizedNewModel = useMemo(
    () =>
      normalizeModelValue({
        model: newModel,
        contextWindowTokens: newModelContextWindowTokens,
      }),
    [newModel, newModelContextWindowTokens],
  )
  const canCreateModel =
    selectedProvider !== null &&
    normalizedNewModel.model.length > 0 &&
    normalizedNewModel.contextWindowTokensError === null &&
    modelBusyId === null

  useEffect(() => {
    setNewModel(selectedProviderId === 'new' ? DEFAULT_PROVIDER_MODEL : '')
    setNewModelContextWindowTokens('')
    setNewModelError(null)
  }, [selectedProviderId])

  async function handleCreateModel(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!canCreateModel) return
    await onCreateModel(normalizedNewModel)
    setNewModel('')
    setNewModelContextWindowTokens('')
  }

  async function handleSaveProviderWithInitialModel(value: {
    profile_name: string
    base_url: string
    api_key: string
  }) {
    if (selectedProvider) {
      await onSaveProvider(value)
      return
    }
    if (!normalizedNewModel.model) {
      setNewModelError('请填写首个模型 ID。')
      return
    }
    if (normalizedNewModel.contextWindowTokensError) {
      return
    }
    setNewModelError(null)
    await onSaveProvider({
      ...value,
      initial_model: normalizedNewModel.model,
      initial_context_window_tokens: normalizedNewModel.context_window_tokens,
    })
  }

  return (
    <div className='grid gap-4 lg:grid-cols-[clamp(220px,26vw,320px)_minmax(0,1fr)]'>
      <Card className='bg-card/80 py-0 shadow-none'>
        <CardHeader className='py-4'>
          <div className='flex items-start justify-between gap-3'>
            <div>
              <CardTitle>服务商账号</CardTitle>
              <CardDescription>同一账号下可配置多个模型。</CardDescription>
            </div>
            <Button
              onClick={onNewProvider}
              size='sm'
              type='button'
              variant={isCreatingNew ? 'secondary' : 'default'}
            >
              <Plus className='mr-1 h-4 w-4' />
              新建
            </Button>
          </div>
        </CardHeader>

        <CardContent className='space-y-3 py-2'>
          {isCreatingNew ? (
            <div className='rounded-xl bg-accent/35 px-3 py-2 text-sm text-foreground'>
              正在创建新服务商。填写首个模型后可一次完成创建。
            </div>
          ) : null}

          {providers.length === 0 ? (
            <div className='rounded-xl px-3 py-4 text-sm text-muted-foreground'>
              还没有服务商配置，点击右上角“新建”开始创建。
            </div>
          ) : (
            <div className='max-h-[52dvh] space-y-2 overflow-y-auto pr-1'>
              {providers.map((provider) => (
                <button
                  className={cn(
                    'w-full rounded-xl p-3 text-left transition-colors',
                    selectedProviderId === provider.id
                      ? 'bg-accent/45 text-foreground'
                      : 'bg-background/85 text-foreground hover:bg-accent/20',
                  )}
                  key={provider.id}
                  onClick={() => setSelectedProviderId(provider.id)}
                  type='button'
                >
                  <div className='flex items-center justify-between gap-2'>
                    <p className='truncate text-sm font-medium'>{provider.name}</p>
                    {selectedProviderId === provider.id ? (
                      <CheckCircle2 className='h-4 w-4 shrink-0 text-foreground/80' />
                    ) : (
                      <Server className='h-4 w-4 shrink-0 text-muted-foreground' />
                    )}
                  </div>
                  <p className='mt-1 truncate text-xs text-muted-foreground'>
                    {formatProviderHost(provider.base_url)}
                  </p>
                  <Badge className='mt-2 bg-card text-foreground'>
                    {provider.models.length} 个模型
                  </Badge>
                </button>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <Card className='bg-card/80 py-0 shadow-none'>
        <CardHeader className='py-4'>
          <div className='flex items-start justify-between gap-3'>
            <div>
              <CardTitle>
                {selectedProvider ? `编辑服务商: ${selectedProvider.name}` : '新建服务商'}
              </CardTitle>
              <CardDescription>在同一个区域完成服务商连接与模型配置。</CardDescription>
            </div>
            <Badge className='gap-1 bg-background text-foreground'>
              <PencilLine className='h-3.5 w-3.5' />
              {selectedProvider ? '编辑模式' : '新建模式'}
            </Badge>
          </div>
        </CardHeader>

        <CardContent className='space-y-6 py-2'>
          <div className='space-y-4'>
            <ProviderForm
              busy={accountBusy}
              initialValue={selectedProvider}
              onSubmit={handleSaveProviderWithInitialModel}
              submitLabel={selectedProvider ? '保存服务商' : '创建服务商并添加首个模型'}
            />
          </div>

          <div className='space-y-4'>
            {!selectedProvider ? (
              <div className='rounded-xl bg-muted/30 p-2'>
                <p className='text-xs uppercase tracking-[0.18em] text-muted-foreground'>
                  首个模型
                </p>
                <div className='mt-2 space-y-2'>
                  <div className='flex items-center gap-2 overflow-x-auto'>
                    <Input
                      className='min-w-[220px] flex-1'
                      disabled={accountBusy}
                      onChange={(event) => {
                        setNewModel(event.target.value)
                        if (newModelError) {
                          setNewModelError(null)
                        }
                      }}
                      placeholder={DEFAULT_PROVIDER_MODEL}
                      value={newModel}
                    />
                    <ContextWindowAdvancedControl
                      align='start'
                      disabled={accountBusy}
                      error={normalizedNewModel.contextWindowTokensError}
                      onChange={setNewModelContextWindowTokens}
                      value={newModelContextWindowTokens}
                    />
                    {normalizedNewModel.context_window_tokens === null ? null : (
                      <Badge className='shrink-0 bg-background text-foreground'>
                        {formatContextWindowTokens(normalizedNewModel.context_window_tokens)} tokens
                      </Badge>
                    )}
                  </div>
                  {newModelError ? (
                    <p className='text-xs text-destructive'>{newModelError}</p>
                  ) : null}
                  {normalizedNewModel.contextWindowTokensError ? (
                    <p className='text-xs text-destructive'>
                      {normalizedNewModel.contextWindowTokensError}
                    </p>
                  ) : (
                    <p className='text-xs text-muted-foreground'>留空表示使用全局默认窗口。</p>
                  )}
                </div>
                <p className='mt-2 text-xs text-muted-foreground'>保存服务商时会一并创建该模型。</p>
              </div>
            ) : (
              <>
                {selectedProvider.models.length === 0 ? (
                  <div className='rounded-xl px-3 py-3 text-sm text-muted-foreground'>
                    当前服务商还没有模型，请先添加一个。
                  </div>
                ) : (
                  <div className='space-y-2'>
                    {selectedProvider.models.map((model) => (
                      <ProviderModelItem
                        key={model.id}
                        model={model}
                        modelBusyId={modelBusyId}
                        onDeleteModel={onDeleteModel}
                        onTestModel={onTestModel}
                        onUpdateModel={onUpdateModel}
                        providerId={selectedProvider.id}
                      />
                    ))}
                  </div>
                )}

                <form
                  className='rounded-xl bg-muted/30 p-2'
                  onSubmit={(event) => void handleCreateModel(event)}
                >
                  <div className='flex items-center gap-2 overflow-x-auto'>
                    <Input
                      className='min-w-[220px] flex-1'
                      disabled={modelBusyId !== null}
                      onChange={(event) => setNewModel(event.target.value)}
                      placeholder={DEFAULT_PROVIDER_MODEL}
                      value={newModel}
                    />
                    <ContextWindowAdvancedControl
                      disabled={modelBusyId !== null}
                      error={normalizedNewModel.contextWindowTokensError}
                      onChange={setNewModelContextWindowTokens}
                      value={newModelContextWindowTokens}
                    />
                    {normalizedNewModel.context_window_tokens === null ? null : (
                      <Badge className='shrink-0 bg-background text-foreground'>
                        {formatContextWindowTokens(normalizedNewModel.context_window_tokens)} tokens
                      </Badge>
                    )}
                    <Button disabled={!canCreateModel} size='sm' type='submit'>
                      {modelBusyId === 'create' ? '添加中...' : '添加模型'}
                    </Button>
                  </div>
                  {normalizedNewModel.contextWindowTokensError ? (
                    <p className='mt-2 text-xs text-destructive'>
                      {normalizedNewModel.contextWindowTokensError}
                    </p>
                  ) : (
                    <p className='mt-2 text-xs text-muted-foreground'>默认使用全局窗口。</p>
                  )}
                </form>
              </>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
