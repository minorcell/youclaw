import { useEffect, useMemo, useState, type FormEvent } from "react"
import { useNavigate } from "react-router-dom"
import { FolderOpen, MessageSquare, ShieldCheck } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { getAppClient } from "@/lib/app-client"
import type { ChatSession, ProviderAccount, ProviderModel } from "@/lib/types"
import { useAppStore } from "@/store/app-store"

export function ProviderOnboardingPage() {
  const navigate = useNavigate()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const providerAccounts = useAppStore((state) => state.providerAccounts)
  const sessions = useAppStore((state) => state.sessions)
  const firstProvider = useMemo(() => providerAccounts[0] ?? null, [providerAccounts])
  const firstModel = useMemo(() => firstProvider?.models[0] ?? null, [firstProvider])

  const initial = useMemo(
    () => ({
      profile_name: firstProvider?.name ?? "deepseek",
      base_url: firstProvider?.base_url ?? "https://api.deepseek.com",
      api_key: firstProvider?.api_key ?? "",
      model: firstModel?.model ?? "deepseek-chat",
    }),
    [firstProvider, firstModel],
  )

  const [form, setForm] = useState(initial)

  useEffect(() => {
    setForm(initial)
  }, [initial])

  async function ensureSession(providerProfileId: string, existingSessions: ChatSession[]) {
    const client = getAppClient()
    if (existingSessions.length > 0) {
      const target = existingSessions[0]
      if (!target.provider_profile_id) {
        await client.request("sessions.bind_provider", {
          session_id: target.id,
          provider_profile_id: providerProfileId,
        })
      }
      navigate(`/chat/${target.id}`)
      return
    }

    const created = await client.request<ChatSession>("sessions.create", {
      provider_profile_id: providerProfileId,
    })
    navigate(`/chat/${created.id}`)
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setError(null)
    setBusy(true)
    try {
      const client = getAppClient()
      const provider = firstProvider
        ? await client.request<ProviderAccount>("providers.update", {
            id: firstProvider.id,
            profile_name: form.profile_name,
            base_url: form.base_url,
            api_key: form.api_key,
          })
        : await client.request<ProviderAccount>("providers.create", {
            profile_name: form.profile_name,
            base_url: form.base_url,
            api_key: form.api_key,
          })

      const targetModel = firstModel
        ? await client.request<ProviderModel>("providers.models.update", {
            id: firstModel.id,
            model_name: form.model,
            model: form.model,
          })
        : await client.request<ProviderModel>("providers.models.create", {
            provider_id: provider.id,
            model_name: form.model,
            model: form.model,
          })

      await ensureSession(targetModel.id, sessions)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <main className="box-border flex min-h-dvh flex-col items-center justify-center bg-background px-4 py-12 text-foreground">
      {/* Brand */}
      <div className="mb-10 select-none text-center">
        <p className="text-xs uppercase tracking-[0.28em] text-muted-foreground">
          运行在本地的 AI 助手
        </p>
        <h1 className="mt-2 font-serif text-[3.2rem] font-semibold leading-none tracking-tight text-[#224c37]">
          BgtClaw
        </h1>
        <p className="mt-3 text-sm text-muted-foreground">
          连接你的 AI 模型，开始与本地 Agent 协作
        </p>
      </div>

      {/* Feature highlights */}
      <div className="mb-8 grid w-full max-w-140 select-none grid-cols-3 gap-3">
        {[
          {
            icon: <FolderOpen className="h-4 w-4" />,
            label: "文件系统工具",
            desc: "读写本地文件",
          },
          {
            icon: <MessageSquare className="h-4 w-4" />,
            label: "多会话管理",
            desc: "独立上下文",
          },
          {
            icon: <ShieldCheck className="h-4 w-4" />,
            label: "完全本地",
            desc: "无登录，无云端",
          },
        ].map(({ icon, label, desc }) => (
          <Card
            key={label}
            className="flex flex-col items-start gap-2 rounded-2xl border-border/60 bg-card/50 px-4 py-3 shadow-none"
          >
            <span className="text-muted-foreground">{icon}</span>
            <div>
              <p className="text-xs font-medium text-foreground">{label}</p>
              <p className="mt-0.5 text-xs text-muted-foreground">{desc}</p>
            </div>
          </Card>
        ))}
      </div>

      {/* Config form */}
      <div className="w-full max-w-140">
        <Card className="rounded-3xl border-border/60 p-6 shadow-none">
          <p className="mb-1 text-xs uppercase tracking-[0.22em] text-muted-foreground">
            第一步
          </p>
          <h2 className="mb-5 font-serif text-lg font-semibold text-foreground">
            配置 AI Provider
          </h2>

          <form className="space-y-4" onSubmit={handleSubmit}>
            <div className="space-y-2">
              <Label className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
                Provider 名称
              </Label>
              <Input
                value={form.profile_name}
                onChange={(e) =>
                  setForm((c) => ({ ...c, profile_name: e.target.value }))
                }
                placeholder="OpenAI-compatible"
              />
            </div>
            <div className="space-y-2">
              <Label className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
                Base URL
              </Label>
              <Input
                value={form.base_url}
                onChange={(e) =>
                  setForm((c) => ({ ...c, base_url: e.target.value }))
                }
                placeholder="https://api.deepseek.com"
              />
            </div>
            <div className="space-y-2">
              <Label className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
                API Key
              </Label>
              <Input
                type="password"
                value={form.api_key}
                onChange={(e) =>
                  setForm((c) => ({ ...c, api_key: e.target.value }))
                }
                placeholder="sk-..."
              />
            </div>
            <div className="space-y-2">
              <Label className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
                模型
              </Label>
              <Input
                value={form.model}
                onChange={(e) =>
                  setForm((c) => ({ ...c, model: e.target.value }))
                }
                placeholder="deepseek-chat"
              />
            </div>
            {error ? (
              <div className="rounded-xl border border-destructive/30 bg-destructive/10 px-3 py-2">
                <p className="text-xs text-destructive">{error}</p>
              </div>
            ) : null}
            <Button className="w-full" disabled={busy} type="submit">
              {busy ? "连接中..." : "开始使用"}
            </Button>
          </form>
        </Card>
      </div>
    </main>
  )
}
