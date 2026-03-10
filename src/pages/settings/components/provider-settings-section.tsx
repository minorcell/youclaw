import { Plus, Server } from "lucide-react"

import { ProviderForm } from "@/pages/settings/components/provider-form"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { ScrollArea } from "@/components/ui/scroll-area"
import type { ProviderProfile } from "@/lib/types"
import { cn } from "@/lib/utils"

interface ProviderSettingsSectionProps {
  providers: ProviderProfile[]
  selectedProvider: ProviderProfile | null
  selectedProviderId: string | "new"
  setSelectedProviderId: (id: string | "new") => void
  onNewProvider: () => void
  onSaveProvider: (value: {
    profile_name: string
    base_url: string
    api_key: string
    model: string
  }) => Promise<void>
  busy: boolean
}

export function ProviderSettingsSection({
  providers,
  selectedProvider,
  selectedProviderId,
  setSelectedProviderId,
  onNewProvider,
  onSaveProvider,
  busy,
}: ProviderSettingsSectionProps) {
  return (
    <div className="grid gap-4 lg:grid-cols-[360px_minmax(0,1fr)]">
      <Card className="border-border/70 bg-background/70 py-0 shadow-none">
        <CardHeader className="border-b pb-4">
          <div className="flex items-center justify-between">
            <p className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
              Profiles
            </p>
            <Button onClick={onNewProvider} size="sm" type="button">
              <Plus className="mr-1 h-4 w-4" />
              New
            </Button>
          </div>
        </CardHeader>

        <CardContent className="p-4">
          <ScrollArea className="max-h-[52dvh] pr-2">
            <div className="space-y-2">
              {providers.map((provider) => (
                <Button
                  className={cn(
                    "h-auto w-full justify-start rounded-xl border p-3 text-left",
                    selectedProviderId === provider.id
                      ? "border-border bg-accent/70 text-foreground"
                      : "border-border bg-card hover:bg-accent/50",
                  )}
                  key={provider.id}
                  onClick={() => setSelectedProviderId(provider.id)}
                  type="button"
                  variant="ghost"
                >
                  <div className="w-full">
                    <div className="flex items-center justify-between gap-2">
                      <p className="truncate text-sm font-medium">{provider.name}</p>
                      <Server className="h-4 w-4 shrink-0 text-muted-foreground" />
                    </div>
                    <p className="mt-1 truncate text-xs text-muted-foreground">
                      {provider.base_url}
                    </p>
                    <Badge className="mt-2 border-border bg-background text-foreground">
                      {provider.model}
                    </Badge>
                  </div>
                </Button>
              ))}

              {providers.length === 0 ? (
                <Card className="border-dashed border-border bg-transparent py-0 text-sm text-muted-foreground shadow-none">
                  <CardContent className="p-4">还没有 provider，先创建一个。</CardContent>
                </Card>
              ) : null}
            </div>
          </ScrollArea>
        </CardContent>
      </Card>

      <div className="space-y-4">
        <Card className="border-border/70 bg-background/70 py-0 shadow-none">
          <CardHeader>
            <div className="flex items-center justify-between gap-3">
              <CardTitle>
                {selectedProvider
                  ? selectedProvider.name
                  : "New OpenAI-compatible provider"}
              </CardTitle>
              {selectedProvider ? (
                <Badge className="border-border bg-card text-foreground">
                  {selectedProvider.model}
                </Badge>
              ) : null}
            </div>
          </CardHeader>
        </Card>

        <ProviderForm
          busy={busy}
          initialValue={selectedProvider}
          onSubmit={onSaveProvider}
        />
      </div>
    </div>
  )
}
