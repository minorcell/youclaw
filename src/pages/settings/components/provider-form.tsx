import { useEffect, useMemo, useState, type FormEvent } from "react"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import type { ProviderProfile } from "@/lib/types"

interface ProviderFormProps {
  initialValue?: ProviderProfile | null
  busy?: boolean
  onSubmit: (value: {
    profile_name: string
    base_url: string
    api_key: string
    model: string
  }) => Promise<void>
}

export function ProviderForm({ initialValue, busy, onSubmit }: ProviderFormProps) {
  const initial = useMemo(
    () => ({
      profile_name: initialValue?.name ?? "",
      base_url: initialValue?.base_url ?? "https://api.openai.com/v1",
      api_key: initialValue?.api_key ?? "",
      model: initialValue?.model ?? "gpt-4o-mini",
    }),
    [initialValue],
  )

  const [form, setForm] = useState(initial)

  useEffect(() => {
    setForm(initial)
  }, [initial])

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    await onSubmit(form)
  }

  return (
    <Card className="p-6">
      <form className="space-y-4" onSubmit={handleSubmit}>
        <div className="space-y-2">
          <Label className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
            Profile Name
          </Label>
          <Input
            value={form.profile_name}
            onChange={(event) => setForm((current) => ({ ...current, profile_name: event.target.value }))}
            placeholder="OpenAI-compatible"
          />
        </div>
        <div className="space-y-2">
          <Label className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
            Base URL
          </Label>
          <Input
            value={form.base_url}
            onChange={(event) => setForm((current) => ({ ...current, base_url: event.target.value }))}
            placeholder="https://api.openai.com/v1"
          />
        </div>
        <div className="space-y-2">
          <Label className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
            API Key
          </Label>
          <Input
            type="password"
            value={form.api_key}
            onChange={(event) => setForm((current) => ({ ...current, api_key: event.target.value }))}
            placeholder="sk-..."
          />
        </div>
        <div className="space-y-2">
          <Label className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
            Model
          </Label>
          <Input
            value={form.model}
            onChange={(event) => setForm((current) => ({ ...current, model: event.target.value }))}
            placeholder="gpt-4o-mini"
          />
        </div>
        <Alert className="rounded-3xl border-amber-500/30 bg-amber-500/10 text-amber-900">
          <AlertDescription className="text-sm text-amber-900">
            API Key 当前按你的要求以明文 JSON 存本地，仅适合本机使用。
          </AlertDescription>
        </Alert>
        <Button className="w-full" disabled={busy} type="submit">
          {busy ? "Testing connection..." : initialValue ? "Update Provider" : "Save And Test"}
        </Button>
      </form>
    </Card>
  )
}
