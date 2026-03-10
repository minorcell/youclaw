import { Server, SlidersHorizontal, X, type LucideIcon } from "lucide-react"
import { useEffect, useMemo, useState } from "react"
import { Navigate } from "react-router-dom"

import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { ScrollArea } from "@/components/ui/scroll-area"
import { useToastContext } from "@/contexts/toast-context"
import { ProviderSettingsSection } from "@/pages/settings/components/provider-settings-section"
import { ThemeSettingsSection } from "@/pages/settings/components/theme-settings-section"
import { getAppClient } from "@/lib/app-client"
import { flattenProviderProfiles } from "@/lib/provider-profiles"
import type { ProviderAccount, ProviderModel } from "@/lib/types"
import { cn } from "@/lib/utils"
import { useAppStore } from "@/store/app-store"
import {
  useSettingsStore,
  type SettingsSection,
} from "@/store/settings-store"

interface SettingsModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

interface SettingsSectionMeta {
  label: string
  description: string
  icon: LucideIcon
}

const sectionMeta: Record<SettingsSection, SettingsSectionMeta> = {
  general: {
    label: "通用设置",
    description: "管理界面模式与整体视觉风格",
    icon: SlidersHorizontal,
  },
  providers: {
    label: "模型服务商",
    description: "创建和编辑 OpenAI 兼容的服务商配置",
    icon: Server,
  },
}

const sections: SettingsSection[] = ["general", "providers"]

function errorMessageFromUnknown(error: unknown): string {
  if (typeof error === "string") {
    return error
  }
  if (error instanceof Error) {
    return error.message
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message
  }
  return "操作失败，请稍后重试。"
}

export function SettingsPage() {
  const initialized = useAppStore((state) => state.initialized)
  const providerAccounts = useAppStore((state) => state.providerAccounts)
  const sessions = useAppStore((state) => state.sessions)
  const activeSessionId = useAppStore((state) => state.activeSessionId)
  const lastOpenedSessionId = useAppStore((state) => state.lastOpenedSessionId)
  const providers = useMemo(
    () => flattenProviderProfiles(providerAccounts),
    [providerAccounts],
  )

  if (!initialized) {
    return <div className="h-screen overflow-hidden bg-background" />
  }

  if (providers.length === 0) {
    return <Navigate replace to="/welcome/provider" />
  }

  const targetSessionId =
    activeSessionId ?? lastOpenedSessionId ?? sessions[0]?.id ?? null

  if (!targetSessionId) {
    return <Navigate replace to="/?settings=1" />
  }

  return <Navigate replace to={`/chat/${targetSessionId}?settings=1`} />
}

export function SettingsModal({ open, onOpenChange }: SettingsModalProps) {
  const providerAccounts = useAppStore((state) => state.providerAccounts)
  const section = useSettingsStore((state) => state.settingsSection)
  const setSection = useSettingsStore((state) => state.setSettingsSection)
  const selectedProviderId = useSettingsStore((state) => state.selectedProviderId)
  const setSelectedProviderId = useSettingsStore(
    (state) => state.setSelectedProviderId,
  )
  const resetSettingsUiState = useSettingsStore(
    (state) => state.resetSettingsUiState,
  )
  const { success: toastSuccess, error: toastError } = useToastContext()
  const [accountBusy, setAccountBusy] = useState(false)
  const [modelBusyId, setModelBusyId] = useState<string | null>(null)

  const themeMode = useSettingsStore((state) => state.mode)
  const themePreset = useSettingsStore((state) => state.preset)
  const setThemeMode = useSettingsStore((state) => state.setMode)
  const setThemePreset = useSettingsStore((state) => state.setPreset)

  useEffect(() => {
    if (!open) {
      setAccountBusy(false)
      setModelBusyId(null)
      resetSettingsUiState()
      return
    }

    if (providerAccounts.length === 0) {
      setSelectedProviderId("new")
      return
    }

    if (selectedProviderId === "new") {
      setSelectedProviderId(providerAccounts[0].id)
      return
    }

    const hasSelectedProvider = providerAccounts.some(
      (provider) => provider.id === selectedProviderId,
    )
    if (!hasSelectedProvider) {
      setSelectedProviderId(providerAccounts[0].id)
    }
  }, [
    open,
    providerAccounts,
    resetSettingsUiState,
    selectedProviderId,
    setSelectedProviderId,
  ])

  const selectedProvider = useMemo<ProviderAccount | null>(() => {
    if (selectedProviderId === "new") {
      return null
    }
    return (
      providerAccounts.find((provider) => provider.id === selectedProviderId) ??
      null
    )
  }, [providerAccounts, selectedProviderId])

  const activeSection = sectionMeta[section]

  function handleSectionChange(nextSection: SettingsSection) {
    setSection(nextSection)
  }

  function handleProviderSelection(nextProviderId: string | "new") {
    setSelectedProviderId(nextProviderId)
  }

  function handleThemeModeChange(value: string | null) {
    if (!value) return
    if (value === "white" || value === "black" || value === "custom") {
      setThemeMode(value)
    }
  }

  async function handleSaveProvider(value: {
    profile_name: string
    base_url: string
    api_key: string
    initial_model?: string
  }) {
    setAccountBusy(true)
    try {
      if (selectedProvider) {
        const updated = await getAppClient().request<ProviderAccount>(
          "providers.update",
          {
            id: selectedProvider.id,
            ...value,
          },
        )
        handleProviderSelection(updated.id)
        toastSuccess("服务商配置已更新。")
        return
      }
      const created = await getAppClient().request<ProviderAccount>(
        "providers.create",
        {
          profile_name: value.profile_name,
          base_url: value.base_url,
          api_key: value.api_key,
        },
      )
      handleProviderSelection(created.id)
      const initialModelId = value.initial_model?.trim()
      if (initialModelId) {
        await getAppClient().request<ProviderModel>("providers.models.create", {
          provider_id: created.id,
          model_name: initialModelId,
          model: initialModelId,
        })
        toastSuccess("服务商与首个模型已创建。")
      } else {
        toastSuccess("服务商已创建。")
      }
    } catch (error) {
      toastError(errorMessageFromUnknown(error))
    } finally {
      setAccountBusy(false)
    }
  }

  async function handleCreateModel(value: { model: string }) {
    if (!selectedProvider) return
    setModelBusyId("create")
    try {
      await getAppClient().request<ProviderModel>("providers.models.create", {
        provider_id: selectedProvider.id,
        model_name: value.model,
        model: value.model,
      })
      toastSuccess("模型已添加。")
    } catch (error) {
      toastError(errorMessageFromUnknown(error))
    } finally {
      setModelBusyId(null)
    }
  }

  async function handleUpdateModel(modelId: string, value: { model: string }) {
    setModelBusyId(`save:${modelId}`)
    try {
      await getAppClient().request<ProviderModel>("providers.models.update", {
        id: modelId,
        model_name: value.model,
        model: value.model,
      })
      toastSuccess("模型配置已更新。")
    } catch (error) {
      toastError(errorMessageFromUnknown(error))
    } finally {
      setModelBusyId(null)
    }
  }

  async function handleDeleteModel(modelId: string) {
    if (selectedProvider && selectedProvider.models.length <= 1) {
      toastError("至少保留一个模型。请先添加新模型，再删除当前模型。")
      return
    }
    setModelBusyId(`delete:${modelId}`)
    try {
      await getAppClient().request("providers.models.delete", {
        id: modelId,
      })
      toastSuccess("模型已移除。")
    } catch (error) {
      toastError(errorMessageFromUnknown(error))
    } finally {
      setModelBusyId(null)
    }
  }

  async function handleTestModel(value: {
    provider_id: string
    model: string
    model_id?: string
  }) {
    const busyKey = value.model_id ? `test:${value.model_id}` : `test:${value.model}`
    setModelBusyId(busyKey)
    try {
      await getAppClient().request("providers.models.test", value)
      toastSuccess("模型连接测试成功。")
    } catch (error) {
      toastError(errorMessageFromUnknown(error))
    } finally {
      setModelBusyId(null)
    }
  }

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent
        className="h-[min(860px,calc(100dvh-1.5rem))] w-[min(1120px,calc(100vw-1.5rem))] max-w-[min(1120px,calc(100vw-1.5rem))] overflow-hidden rounded-3xl border border-border/70 bg-card p-0 shadow-[0_36px_110px_-56px_rgba(0,0,0,0.45)] sm:max-w-[min(1120px,calc(100vw-1.5rem))]"
        showCloseButton={false}
      >
        <DialogTitle className="sr-only">设置</DialogTitle>

        <div className="flex h-full min-h-0 flex-col select-none">
          <header className="flex items-start justify-between border-b border-border/70 bg-card/80 px-4 py-4 sm:px-6">
            <div>
              <p className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
                设置
              </p>
              <h3 className="mt-1 text-xl font-semibold tracking-tight sm:text-2xl">
                {activeSection.label}
              </h3>
              <p className="mt-1 text-sm text-muted-foreground">
                {activeSection.description}
              </p>
            </div>
            <Button
              aria-label="Close settings"
              onClick={() => onOpenChange(false)}
              size="icon-sm"
              type="button"
              variant="ghost"
            >
              <X className="h-5 w-5" />
            </Button>
          </header>

          <div className="grid min-h-0 flex-1 bg-background/70 md:grid-cols-[248px_minmax(0,1fr)]">
            <aside className="hidden border-r border-border/70 bg-muted/35 p-3 md:block">
              <nav aria-label="设置导航" className="space-y-2">
                {sections.map((itemId) => {
                  const item = sectionMeta[itemId]
                  const Icon = item.icon
                  return (
                    <button
                      aria-current={section === itemId ? "page" : undefined}
                      className={cn(
                        "w-full rounded-xl border px-3 py-2.5 text-left transition-colors",
                        section === itemId
                          ? "border-border bg-background text-foreground"
                          : "border-transparent bg-transparent text-muted-foreground hover:border-border/60 hover:bg-background/80 hover:text-foreground",
                      )}
                      key={itemId}
                      onClick={() => handleSectionChange(itemId)}
                      type="button"
                    >
                      <div className="flex items-center gap-2">
                        <Icon className="h-4 w-4" />
                        <span className="text-sm font-medium">{item.label}</span>
                      </div>
                    </button>
                  )
                })}
              </nav>
            </aside>

            <section className="flex min-h-0 flex-1 flex-col">
              <div className="border-b border-border/70 p-3 md:hidden">
                <div className="grid grid-cols-2 gap-2">
                  {sections.map((itemId) => {
                    const item = sectionMeta[itemId]
                    const Icon = item.icon
                    return (
                      <button
                        className={cn(
                          "flex items-center justify-center gap-2 rounded-xl border px-2 py-2 text-sm font-medium transition-colors",
                          section === itemId
                            ? "border-border bg-background text-foreground"
                            : "border-border/50 bg-card/50 text-muted-foreground",
                        )}
                        key={itemId}
                        onClick={() => handleSectionChange(itemId)}
                        type="button"
                      >
                        <Icon className="h-4 w-4" />
                        {item.label}
                      </button>
                    )
                  })}
                </div>
              </div>

              <ScrollArea className="min-h-0 flex-1">
                <div className="px-4 py-5 sm:px-6 sm:py-6">
                  {section === "general" ? (
                    <ThemeSettingsSection
                      mode={themeMode}
                      onModeChange={handleThemeModeChange}
                      onPresetChange={setThemePreset}
                      preset={themePreset}
                    />
                  ) : (
                    <ProviderSettingsSection
                      accountBusy={accountBusy}
                      modelBusyId={modelBusyId}
                      onCreateModel={handleCreateModel}
                      onDeleteModel={handleDeleteModel}
                      onNewProvider={() => handleProviderSelection("new")}
                      onSaveProvider={handleSaveProvider}
                      onTestModel={handleTestModel}
                      onUpdateModel={handleUpdateModel}
                      providers={providerAccounts}
                      selectedProvider={selectedProvider}
                      selectedProviderId={selectedProviderId}
                      setSelectedProviderId={handleProviderSelection}
                    />
                  )}
                </div>
              </ScrollArea>
            </section>
          </div>

          <footer className="flex items-center justify-end gap-2 border-t border-border/70 bg-muted/35 px-4 py-3 sm:px-6 sm:py-4">
            <Button onClick={() => onOpenChange(false)} type="button">
              完成
            </Button>
          </footer>
        </div>
      </DialogContent>
    </Dialog>
  )
}
