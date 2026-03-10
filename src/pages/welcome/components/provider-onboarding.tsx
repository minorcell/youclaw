import { useMemo, useState } from "react"
import { useNavigate } from "react-router-dom"

import { ProviderForm } from "@/pages/settings/components"
import { Badge } from "@/components/ui/badge"
import { getAppClient } from "@/lib/app-client"
import type { ChatSession, ProviderProfile } from "@/lib/types"
import { useAppStore } from "@/store/app-store"

export function ProviderOnboardingPage() {
  const navigate = useNavigate()
  const [busy, setBusy] = useState(false)
  const providers = useAppStore((state) => state.providers)
  const sessions = useAppStore((state) => state.sessions)
  const firstProvider = useMemo(() => providers[0] ?? null, [providers])

  async function ensureSession(
    provider: ProviderProfile,
    existingSessions: ChatSession[],
  ) {
    const client = getAppClient()
    if (existingSessions.length > 0) {
      const target = existingSessions[0]
      if (!target.provider_profile_id) {
        await client.request("sessions.bind_provider", {
          session_id: target.id,
          provider_profile_id: provider.id,
        })
      }
      navigate(`/chat/${target.id}`)
      return
    }

    const created = await client.request<ChatSession>("sessions.create", {
      provider_profile_id: provider.id,
    })
    navigate(`/chat/${created.id}`)
  }

  async function handleSubmit(value: {
    profile_name: string
    base_url: string
    api_key: string
    model: string
  }) {
    setBusy(true)
    try {
      const client = getAppClient()
      const provider = firstProvider
        ? await client.request<ProviderProfile>("providers.update", {
            id: firstProvider.id,
            ...value,
          })
        : await client.request<ProviderProfile>("providers.create", value)
      await ensureSession(provider, sessions)
    } finally {
      setBusy(false)
    }
  }

  return (
    <main className="box-border h-[100dvh] overflow-hidden bg-background px-4 py-6 text-foreground md:px-8 md:py-8">
      <div className="mx-auto grid h-full max-w-6xl min-h-0 gap-8 overflow-auto lg:grid-cols-[1.2fr_0.9fr]">
        <section className="rounded-3xl border border-border bg-card/90 p-8 shadow-none md:p-12">
          <Badge className="border-border bg-background/80 text-foreground/80">
            BgtClaw
          </Badge>
          <h1 className="mt-6 max-w-xl text-5xl font-semibold tracking-[-0.04em] text-foreground">
            24/7的智能助理，链接你的生活。
          </h1>
        </section>
        <ProviderForm
          busy={busy}
          initialValue={firstProvider}
          onSubmit={handleSubmit}
        />
      </div>
    </main>
  )
}
