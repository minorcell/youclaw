import { RotateCcw } from 'lucide-react'
import { useEffect, useMemo, useState, type FormEvent } from 'react'

import {
  parseProviderApiKeyInput,
  ProviderApiKeyField,
  type ProviderApiKeyInputValue,
  serializeProviderApiKeyInput,
  validateProviderApiKeyInput,
} from '@/components/provider-api-key-field'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { DEFAULT_PROVIDER_BASE_URL, DEFAULT_PROVIDER_NAME } from '@/lib/provider-defaults'
import type { ProviderAccount } from '@/lib/types'

interface ProviderFormValue {
  profile_name: string
  base_url: string
  api_key: string
}

interface ProviderFormState {
  profile_name: string
  base_url: string
  api_key: ProviderApiKeyInputValue
}

interface ProviderFormProps {
  initialValue?: ProviderAccount | null
  busy?: boolean
  submitLabel?: string
  onSubmit: (value: ProviderFormValue) => Promise<void>
}

function normalizeFormValue(value: ProviderFormState): ProviderFormValue {
  return {
    profile_name: value.profile_name.trim(),
    base_url: value.base_url.trim(),
    api_key: serializeProviderApiKeyInput(value.api_key),
  }
}

function isHttpUrl(value: string): boolean {
  try {
    const url = new URL(value)
    return url.protocol === 'http:' || url.protocol === 'https:'
  } catch {
    return false
  }
}

function sameFormValue(a: ProviderFormValue, b: ProviderFormValue): boolean {
  return a.profile_name === b.profile_name && a.base_url === b.base_url && a.api_key === b.api_key
}

export function ProviderForm({ initialValue, busy, submitLabel, onSubmit }: ProviderFormProps) {
  const initialApiKey = useMemo(
    () => parseProviderApiKeyInput(initialValue?.api_key ?? ''),
    [initialValue?.api_key],
  )

  const initial = useMemo<ProviderFormState>(
    () => ({
      profile_name: initialValue?.name ?? DEFAULT_PROVIDER_NAME,
      base_url: initialValue?.base_url ?? DEFAULT_PROVIDER_BASE_URL,
      api_key: initialApiKey,
    }),
    [initialApiKey, initialValue?.base_url, initialValue?.name],
  )

  const [form, setForm] = useState(initial)
  const [baseUrlTouched, setBaseUrlTouched] = useState(false)

  useEffect(() => {
    setForm(initial)
    setBaseUrlTouched(false)
  }, [initial])

  const normalizedInitial = useMemo(() => normalizeFormValue(initial), [initial])
  const normalizedCurrent = useMemo(() => normalizeFormValue(form), [form])
  const apiKeyError = useMemo(() => validateProviderApiKeyInput(form.api_key), [form.api_key])

  const hasRequiredFields =
    normalizedCurrent.profile_name.length > 0 &&
    normalizedCurrent.base_url.length > 0 &&
    normalizedCurrent.api_key.length > 0

  const isBaseUrlValid = isHttpUrl(normalizedCurrent.base_url)
  const isDirty = !sameFormValue(normalizedCurrent, normalizedInitial)

  const showBaseUrlError = baseUrlTouched && !isBaseUrlValid
  const canSubmit =
    !busy &&
    hasRequiredFields &&
    isBaseUrlValid &&
    apiKeyError === null &&
    (initialValue ? isDirty : true)

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setBaseUrlTouched(true)
    if (!canSubmit) return
    await onSubmit(normalizedCurrent)
  }

  function handleReset() {
    setForm(initial)
    setBaseUrlTouched(false)
  }

  return (
    <form className='space-y-4' onSubmit={handleSubmit}>
      <div className='space-y-2'>
        <Label className='text-xs uppercase tracking-[0.18em] text-muted-foreground'>
          服务商名称
        </Label>
        <Input
          onChange={(event) =>
            setForm((current) => ({
              ...current,
              profile_name: event.target.value,
            }))
          }
          placeholder={DEFAULT_PROVIDER_NAME}
          required
          value={form.profile_name}
        />
      </div>

      <div className='space-y-2'>
        <Label className='text-xs uppercase tracking-[0.18em] text-muted-foreground'>
          Base URL
        </Label>
        <Input
          aria-invalid={showBaseUrlError || undefined}
          onBlur={() => setBaseUrlTouched(true)}
          onChange={(event) =>
            setForm((current) => ({
              ...current,
              base_url: event.target.value,
            }))
          }
          placeholder={DEFAULT_PROVIDER_BASE_URL}
          required
          value={form.base_url}
        />
        {!showBaseUrlError ? (
          <p className='text-xs text-muted-foreground'>
            示例：`https://api.minimaxi.com/v1`
          </p>
        ) : null}
        {showBaseUrlError ? (
          <p className='text-xs text-destructive'>
            请输入完整地址，需以 `http://` 或 `https://` 开头。
          </p>
        ) : null}
      </div>

      <ProviderApiKeyField
        disabled={busy}
        error={apiKeyError}
        onChange={(api_key) =>
          setForm((current) => ({
            ...current,
            api_key,
          }))
        }
        value={form.api_key}
      />

      <div className='flex flex-wrap items-center justify-end gap-2 pt-1'>
        <Button disabled={busy || !isDirty} onClick={handleReset} type='button' variant='outline'>
          <RotateCcw className='mr-1 h-3.5 w-3.5' />
          重置
        </Button>
        <Button disabled={!canSubmit} type='submit'>
          {busy ? '保存中...' : (submitLabel ?? (initialValue ? '保存服务商' : '创建服务商'))}
        </Button>
      </div>
    </form>
  )
}
