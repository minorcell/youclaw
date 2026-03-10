import { Palette, RotateCcw, Server, X, type LucideIcon } from "lucide-react"
import { useEffect, useMemo, useState } from "react"

import { ProviderSettingsSection } from "@/pages/settings/components/provider-settings-section"
import { ThemeSettingsSection } from "@/pages/settings/components/theme-settings-section"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Separator } from "@/components/ui/separator"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { getAppClient } from "@/lib/app-client"
import type { ProviderProfile } from "@/lib/types"
import { cn } from "@/lib/utils"
import { useAppStore } from "@/store/app-store"
import { useThemeStore } from "@/store/theme-store"

type SettingsSection = "theme" | "providers"

interface SettingsModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

const sectionTitle: Record<SettingsSection, string> = {
  theme: "主题",
  providers: "模型服务商",
}

const sections: Array<{
  id: SettingsSection
  label: string
  icon: LucideIcon
}> = [
  { id: "theme", label: "主题", icon: Palette },
  { id: "providers", label: "模型服务商", icon: Server },
]

export function SettingsModal({ open, onOpenChange }: SettingsModalProps) {
  const providers = useAppStore((state) => state.providers)
  const [section, setSection] = useState<SettingsSection>("theme")
  const [selectedProviderId, setSelectedProviderId] = useState<string | "new">(
    "new",
  )
  const [busy, setBusy] = useState(false)

  const themeMode = useThemeStore((state) => state.mode)
  const themePreset = useThemeStore((state) => state.preset)
  const setThemeMode = useThemeStore((state) => state.setMode)
  const setThemePreset = useThemeStore((state) => state.setPreset)
  const resetCustomTheme = useThemeStore((state) => state.resetCustomTheme)

  useEffect(() => {
    if (!open) return
    if (
      selectedProviderId !== "new" &&
      providers.some((provider) => provider.id === selectedProviderId)
    ) {
      return
    }
    setSelectedProviderId(providers[0]?.id ?? "new")
  }, [open, providers, selectedProviderId])

  const selectedProvider = useMemo<ProviderProfile | null>(() => {
    if (selectedProviderId === "new") {
      return null
    }
    return (
      providers.find((provider) => provider.id === selectedProviderId) ?? null
    )
  }, [providers, selectedProviderId])

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
    model: string
  }) {
    setBusy(true)
    try {
      if (selectedProvider) {
        const updated = await getAppClient().request<ProviderProfile>(
          "providers.update",
          {
            id: selectedProvider.id,
            ...value,
          },
        )
        setSelectedProviderId(updated.id)
        return
      }
      const created = await getAppClient().request<ProviderProfile>(
        "providers.create",
        value,
      )
      setSelectedProviderId(created.id)
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent
        className="h-[min(860px,calc(100dvh-2rem))] w-[min(1240px,calc(100vw-2rem))] max-w-[min(1240px,calc(100vw-2rem))] overflow-hidden rounded-3xl border border-border/80 bg-card p-0 shadow-[0_40px_120px_-56px_rgba(0,0,0,0.35)] sm:max-w-[min(1240px,calc(100vw-2rem))]"
        showCloseButton={false}
      >
        <DialogTitle className="sr-only">Settings</DialogTitle>
        <Tabs
          className="h-full min-h-0 w-full gap-0"
          onValueChange={(value) => setSection(value as SettingsSection)}
          orientation="vertical"
          value={section}
        >
          <aside className="flex min-h-0 w-[280px] flex-col border-r border-border bg-muted/45 p-3">
            <div className="px-2 pb-3 pt-2">
              <p className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
                设置
              </p>
              <h2 className="mt-2 text-3xl font-serif tracking-tight">
                BgtClaw
              </h2>
            </div>

            <TabsList
              className="h-auto w-full flex-col items-stretch gap-1 rounded-none bg-transparent p-0"
              variant="line"
            >
              {sections.map((item) => {
                const Icon = item.icon
                return (
                  <TabsTrigger
                    className={cn(
                      "h-11 justify-start rounded-xl border border-transparent px-3 text-sm text-muted-foreground",
                      "data-active:border-border data-active:bg-background data-active:text-foreground data-active:after:opacity-0",
                    )}
                    key={item.id}
                    value={item.id}
                  >
                    <Icon className="mr-2 h-4 w-4" />
                    {item.label}
                  </TabsTrigger>
                )
              })}
            </TabsList>
          </aside>

          <section className="flex min-h-0 flex-1 flex-col">
            <header className="flex items-center justify-between px-6 py-4">
              <h3 className="text-2xl font-semibold tracking-tight">
                {sectionTitle[section]}
              </h3>
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

            <Separator />

            <ScrollArea className="flex-1">
              <div className="px-6 py-5">
                <TabsContent className="mt-0 outline-none" value="theme">
                  <ThemeSettingsSection
                    mode={themeMode}
                    onModeChange={handleThemeModeChange}
                    onPresetChange={setThemePreset}
                    preset={themePreset}
                  />
                </TabsContent>

                <TabsContent className="mt-0 outline-none" value="providers">
                  <ProviderSettingsSection
                    busy={busy}
                    onNewProvider={() => setSelectedProviderId("new")}
                    onSaveProvider={handleSaveProvider}
                    providers={providers}
                    selectedProvider={selectedProvider}
                    selectedProviderId={selectedProviderId}
                    setSelectedProviderId={setSelectedProviderId}
                  />
                </TabsContent>
              </div>
            </ScrollArea>

            <Separator />

            <footer className="flex items-center justify-end gap-2 bg-muted/35 px-6 py-4">
              {section === "theme" ? (
                <Button
                  onClick={resetCustomTheme}
                  type="button"
                  variant="outline"
                >
                  <RotateCcw className="mr-2 h-4 w-4" />
                  恢复默认
                </Button>
              ) : null}
              <Button onClick={() => onOpenChange(false)} type="button">
                完成
              </Button>
            </footer>
          </section>
        </Tabs>
      </DialogContent>
    </Dialog>
  )
}
