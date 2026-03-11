import { useEffect, useMemo, useState, type FormEvent } from 'react'
import { useNavigate } from 'react-router-dom'
import { FolderOpen, MessageSquare, ShieldCheck } from 'lucide-react'

import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useToastContext } from '@/contexts/toast-context'
import { getAppClient } from '@/lib/app-client'
import type { ChatSession, ProviderAccount, ProviderModel } from '@/lib/types'
import { useAppStore } from '@/store/app-store'

function errorMessageFromUnknown(error: unknown): string {
  if (typeof error === 'string') {
    return error
  }
  if (error instanceof Error) {
    return error.message
  }
  if (
    typeof error === 'object' &&
    error !== null &&
    'message' in error &&
    typeof error.message === 'string'
  ) {
    return error.message
  }
  return String(error)
}

export function ProviderOnboardingPage() {
  const navigate = useNavigate()
  const [busy, setBusy] = useState(false)
  const { error: toastError } = useToastContext()
  const providerAccounts = useAppStore((state) => state.providerAccounts)
  const sessions = useAppStore((state) => state.sessions)
  const firstProvider = useMemo(() => providerAccounts[0] ?? null, [providerAccounts])
  const firstModel = useMemo(() => firstProvider?.models[0] ?? null, [firstProvider])

  const initial = useMemo(
    () => ({
      profile_name: firstProvider?.name ?? 'deepseek',
      base_url: firstProvider?.base_url ?? 'https://api.deepseek.com',
      api_key: firstProvider?.api_key ?? '',
      model: firstModel?.model ?? 'deepseek-chat',
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
        await client.request('sessions.bind_provider', {
          session_id: target.id,
          provider_profile_id: providerProfileId,
        })
      }
      navigate(`/chat/${target.id}`)
      return
    }

    const created = await client.request<ChatSession>('sessions.create', {
      provider_profile_id: providerProfileId,
    })
    navigate(`/chat/${created.id}`)
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setBusy(true)
    try {
      const client = getAppClient()
      const provider = firstProvider
        ? await client.request<ProviderAccount>('providers.update', {
            id: firstProvider.id,
            profile_name: form.profile_name,
            base_url: form.base_url,
            api_key: form.api_key,
          })
        : await client.request<ProviderAccount>('providers.create', {
            profile_name: form.profile_name,
            base_url: form.base_url,
            api_key: form.api_key,
          })

      const targetModel = firstModel
        ? await client.request<ProviderModel>('providers.models.update', {
            id: firstModel.id,
            model_name: form.model,
            model: form.model,
          })
        : await client.request<ProviderModel>('providers.models.create', {
            provider_id: provider.id,
            model_name: form.model,
            model: form.model,
          })

      await ensureSession(targetModel.id, sessions)
    } catch (err) {
      toastError(errorMessageFromUnknown(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <main className='box-border flex min-h-dvh flex-col items-center justify-center bg-background px-4 py-12 text-foreground'>
      {/* Brand */}
      <div className='mb-10 select-none text-center'>
        <h1 className='mt-2 font-serif text-[3.2rem] font-semibold leading-none tracking-tight text-[#224c37]'>
          成为你，YouClaw
        </h1>
        <p className='mt-3 text-sm text-muted-foreground'>连接 AI 模型，开始与“你”协作</p>
      </div>

      {/* Config form */}
      <div className='w-full max-w-140'>
        <Card className='rounded-3xl p-6 shadow-none'>
          <h2 className='mb-5 font-serif text-lg font-semibold text-foreground'>配置模型服务商</h2>

          <form className='space-y-4' onSubmit={handleSubmit}>
            <div className='space-y-2'>
              <Label className='text-xs uppercase tracking-[0.18em] text-muted-foreground'>
                服务商名称
              </Label>
              <Input
                value={form.profile_name}
                onChange={(e) => setForm((c) => ({ ...c, profile_name: e.target.value }))}
                placeholder='OpenAI-compatible'
              />
            </div>
            <div className='space-y-2'>
              <Label className='text-xs uppercase tracking-[0.18em] text-muted-foreground'>
                请求地址
              </Label>
              <Input
                value={form.base_url}
                onChange={(e) => setForm((c) => ({ ...c, base_url: e.target.value }))}
                placeholder='https://api.deepseek.com'
              />
            </div>
            <div className='space-y-2'>
              <Label className='text-xs uppercase tracking-[0.18em] text-muted-foreground'>
                API KEY
              </Label>
              <Input
                type='password'
                value={form.api_key}
                onChange={(e) => setForm((c) => ({ ...c, api_key: e.target.value }))}
                placeholder='sk-...'
              />
            </div>
            <div className='space-y-2'>
              <Label className='text-xs uppercase tracking-[0.18em] text-muted-foreground'>
                模型名称
              </Label>
              <Input
                value={form.model}
                onChange={(e) => setForm((c) => ({ ...c, model: e.target.value }))}
                placeholder='deepseek-chat'
              />
            </div>
            <Button className='w-full' disabled={busy} type='submit'>
              {busy ? '连接中...' : '开始使用'}
            </Button>
          </form>
        </Card>
      </div>
    </main>
  )
}
