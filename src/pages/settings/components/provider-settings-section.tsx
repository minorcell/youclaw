import {
  CheckCircle2,
  FlaskConical,
  Loader2,
  PencilLine,
  Plus,
  Save,
  Server,
  Trash2,
} from "lucide-react"
import { useEffect, useMemo, useState, type FormEvent } from "react"

import { ProviderForm } from "@/pages/settings/components/provider-form"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import type { ProviderAccount, ProviderModel } from "@/lib/types"
import { cn } from "@/lib/utils"

interface ProviderSettingsSectionProps {
  providers: ProviderAccount[]
  selectedProvider: ProviderAccount | null
  selectedProviderId: string | "new"
  setSelectedProviderId: (id: string | "new") => void
  onNewProvider: () => void
  onSaveProvider: (value: {
    profile_name: string
    base_url: string
    api_key: string
    initial_model?: string
  }) => Promise<void>
  onCreateModel: (value: { model: string }) => Promise<void>
  onUpdateModel: (modelId: string, value: { model: string }) => Promise<void>
  onDeleteModel: (modelId: string) => Promise<void>
  onTestModel: (value: {
    provider_id: string
    model: string
    model_id?: string
  }) => Promise<void>
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

function normalizeModelValue(value: { model: string }) {
  return {
    model: value.model.trim(),
  }
}

function sameModelValue(left: { model: string }, right: { model: string }) {
  return left.model === right.model
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
  onUpdateModel: (modelId: string, value: { model: string }) => Promise<void>
  onDeleteModel: (modelId: string) => Promise<void>
  onTestModel: (value: {
    provider_id: string
    model: string
    model_id?: string
  }) => Promise<void>
}) {
  const initial = useMemo(() => ({ model: model.model }), [model.model])

  const [form, setForm] = useState(initial)

  useEffect(() => {
    setForm(initial)
  }, [initial])

  const normalizedInitial = useMemo(
    () => normalizeModelValue(initial),
    [initial],
  )
  const normalizedCurrent = useMemo(() => normalizeModelValue(form), [form])
  const isDirty = !sameModelValue(normalizedCurrent, normalizedInitial)
  const canSubmit = normalizedCurrent.model.length > 0
  const isSaving = modelBusyId === `save:${model.id}`
  const isTesting = modelBusyId === `test:${model.id}`
  const isDeleting = modelBusyId === `delete:${model.id}`
  const isBusy = isSaving || isTesting || isDeleting

  return (
    <div className="rounded-xl border border-border/70 bg-background/85 p-2">
      <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-end">
        <div className="space-y-1.5">
          <Input
            disabled={isBusy}
            onChange={(event) =>
              setForm((current) => ({
                ...current,
                model: event.target.value,
              }))
            }
            placeholder="deepseek-chat"
            value={form.model}
          />
        </div>

        <TooltipProvider>
          <div className="flex items-center gap-1">
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button
                    aria-label="保存模型配置"
                    disabled={!canSubmit || !isDirty || isBusy}
                    onClick={() => void onUpdateModel(model.id, normalizedCurrent)}
                    size="icon-sm"
                    type="button"
                  >
                    {isSaving ? (
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    ) : (
                      <Save className="h-3.5 w-3.5" />
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
                    aria-label="测试模型连接"
                    disabled={!canSubmit || isBusy}
                    onClick={() =>
                      void onTestModel({
                        provider_id: providerId,
                        model: normalizedCurrent.model,
                        model_id: model.id,
                      })
                    }
                    size="icon-sm"
                    type="button"
                    variant="outline"
                  >
                    {isTesting ? (
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    ) : (
                      <FlaskConical className="h-3.5 w-3.5" />
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
                    aria-label="删除模型配置"
                    className="text-destructive hover:text-destructive"
                    disabled={isBusy}
                    onClick={() => void onDeleteModel(model.id)}
                    size="icon-sm"
                    type="button"
                    variant="ghost"
                  >
                    {isDeleting ? (
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    ) : (
                      <Trash2 className="h-3.5 w-3.5" />
                    )}
                  </Button>
                }
              />
              <TooltipContent>删除</TooltipContent>
            </Tooltip>
          </div>
        </TooltipProvider>
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
  const isCreatingNew = selectedProviderId === "new"
  const [newModel, setNewModel] = useState("")
  const [newModelError, setNewModelError] = useState<string | null>(null)
  const normalizedNewModel = useMemo(
    () => normalizeModelValue({ model: newModel }),
    [newModel],
  )
  const canCreateModel =
    selectedProvider !== null &&
    normalizedNewModel.model.length > 0 &&
    modelBusyId === null

  useEffect(() => {
    setNewModel("")
    setNewModelError(null)
  }, [selectedProviderId])

  async function handleCreateModel(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!canCreateModel) return
    await onCreateModel(normalizedNewModel)
    setNewModel("")
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
      setNewModelError("请填写首个模型 ID。")
      return
    }
    setNewModelError(null)
    await onSaveProvider({
      ...value,
      initial_model: normalizedNewModel.model,
    })
  }

  return (
    <div className="grid gap-4 xl:grid-cols-[336px_minmax(0,1fr)]">
      <Card className="border border-border/70 bg-card/80 py-0 shadow-none">
        <CardHeader className="border-b border-border/70 py-4">
          <div className="flex items-start justify-between gap-3">
            <div>
              <CardTitle>服务商账号</CardTitle>
              <CardDescription>同一账号下可配置多个模型。</CardDescription>
            </div>
            <Button
              onClick={onNewProvider}
              size="sm"
              type="button"
              variant={isCreatingNew ? "secondary" : "default"}
            >
              <Plus className="mr-1 h-4 w-4" />
              新建
            </Button>
          </div>
        </CardHeader>

        <CardContent className="space-y-3 py-2">
          {isCreatingNew ? (
            <div className="rounded-xl border border-border bg-accent/35 px-3 py-2 text-sm text-foreground">
              正在创建新服务商。填写首个模型后可一次完成创建。
            </div>
          ) : null}

          {providers.length === 0 ? (
            <div className="rounded-xl border border-dashed border-border/70 px-3 py-4 text-sm text-muted-foreground">
              还没有服务商配置，点击右上角“新建”开始创建。
            </div>
          ) : (
            <div className="max-h-[52dvh] space-y-2 overflow-y-auto pr-1">
              {providers.map((provider) => (
                <button
                  className={cn(
                    "w-full rounded-xl border p-3 text-left transition-colors",
                    selectedProviderId === provider.id
                      ? "border-border bg-accent/45 text-foreground"
                      : "border-border/70 bg-background/85 text-foreground hover:bg-accent/20",
                  )}
                  key={provider.id}
                  onClick={() => setSelectedProviderId(provider.id)}
                  type="button"
                >
                  <div className="flex items-center justify-between gap-2">
                    <p className="truncate text-sm font-medium">
                      {provider.name}
                    </p>
                    {selectedProviderId === provider.id ? (
                      <CheckCircle2 className="h-4 w-4 shrink-0 text-foreground/80" />
                    ) : (
                      <Server className="h-4 w-4 shrink-0 text-muted-foreground" />
                    )}
                  </div>
                  <p className="mt-1 truncate text-xs text-muted-foreground">
                    {formatProviderHost(provider.base_url)}
                  </p>
                  <Badge className="mt-2 border-border bg-card text-foreground">
                    {provider.models.length} 个模型
                  </Badge>
                </button>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <Card className="border border-border/70 bg-card/80 py-0 shadow-none">
        <CardHeader className="border-b border-border/70 py-4">
          <div className="flex items-start justify-between gap-3">
            <div>
              <CardTitle>
                {selectedProvider
                  ? `编辑服务商: ${selectedProvider.name}`
                  : "新建服务商"}
              </CardTitle>
              <CardDescription>
                在同一个区域完成服务商连接与模型配置。
              </CardDescription>
            </div>
            <Badge className="gap-1 border-border bg-background text-foreground">
              <PencilLine className="h-3.5 w-3.5" />
              {selectedProvider ? "编辑模式" : "新建模式"}
            </Badge>
          </div>
        </CardHeader>

        <CardContent className="space-y-6 py-2">
          <div className="space-y-4">
            <ProviderForm
              busy={accountBusy}
              initialValue={selectedProvider}
              onSubmit={handleSaveProviderWithInitialModel}
              submitLabel={selectedProvider ? "保存服务商" : "创建服务商并添加首个模型"}
            />
          </div>

          <div className="space-y-4">
            {!selectedProvider ? (
              <div className="rounded-xl border border-border/70 bg-muted/30 p-2">
                <p className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
                  首个模型
                </p>
                <div className="mt-2 grid gap-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-end">
                  <div className="space-y-1.5">
                    <Input
                      disabled={accountBusy}
                      onChange={(event) => {
                        setNewModel(event.target.value)
                        if (newModelError) {
                          setNewModelError(null)
                        }
                      }}
                      placeholder="deepseek-chat"
                      value={newModel}
                    />
                    {newModelError ? (
                      <p className="text-xs text-destructive">{newModelError}</p>
                    ) : null}
                  </div>
                </div>
                <p className="mt-2 text-xs text-muted-foreground">
                  保存服务商时会一并创建该模型。
                </p>
              </div>
            ) : (
              <>
                {selectedProvider.models.length === 0 ? (
                  <div className="rounded-xl border border-dashed border-border/70 px-3 py-3 text-sm text-muted-foreground">
                    当前服务商还没有模型，请先添加一个。
                  </div>
                ) : (
                  <div className="space-y-2">
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
                  className="rounded-xl border border-border/70 bg-muted/30 p-2"
                  onSubmit={(event) => void handleCreateModel(event)}
                >
                  <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-end">
                    <div className="space-y-1.5">
                      <Input
                        disabled={modelBusyId !== null}
                        onChange={(event) => setNewModel(event.target.value)}
                        placeholder="deepseek-chat"
                        value={newModel}
                      />
                    </div>
                    <Button disabled={!canCreateModel} size="sm" type="submit">
                      {modelBusyId === "create" ? "添加中..." : "添加模型"}
                    </Button>
                  </div>
                </form>
              </>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
