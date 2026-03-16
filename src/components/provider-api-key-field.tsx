import { Eye, EyeOff, KeyRound, Variable } from 'lucide-react'
import { useState } from 'react'

import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { cn } from '@/lib/utils'

export type ProviderApiKeyMode = 'direct' | 'env'

export interface ProviderApiKeyInputValue {
  mode: ProviderApiKeyMode
  value: string
}

const API_KEY_ENV_PREFIX = 'env:'
const ENV_VAR_NAME_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/

function stripEnvPrefix(value: string): string {
  const trimmed = value.trim()
  return trimmed.slice(0, API_KEY_ENV_PREFIX.length).toLowerCase() === API_KEY_ENV_PREFIX
    ? trimmed.slice(API_KEY_ENV_PREFIX.length)
    : trimmed
}

export function normalizeProviderApiKeyEnvName(value: string): string {
  return stripEnvPrefix(value).trim().replace(/[;；]+$/, '').trim()
}

export function parseProviderApiKeyInput(
  value: string | null | undefined,
): ProviderApiKeyInputValue {
  const raw = value ?? ''
  const trimmed = raw.trim()
  if (trimmed.slice(0, API_KEY_ENV_PREFIX.length).toLowerCase() === API_KEY_ENV_PREFIX) {
    return {
      mode: 'env',
      value: normalizeProviderApiKeyEnvName(trimmed),
    }
  }
  return {
    mode: 'direct',
    value: raw,
  }
}

export function serializeProviderApiKeyInput(value: ProviderApiKeyInputValue): string {
  if (value.mode === 'env') {
    return `${API_KEY_ENV_PREFIX}${normalizeProviderApiKeyEnvName(value.value)}`
  }
  return value.value.trim()
}

export function validateProviderApiKeyInput(value: ProviderApiKeyInputValue): string | null {
  if (value.mode === 'direct') {
    if (value.value.trim().length === 0) {
      return 'API Key 不能为空。'
    }
    return null
  }

  const envName = normalizeProviderApiKeyEnvName(value.value)
  if (envName.length === 0) {
    return '请输入环境变量名。'
  }
  if (!ENV_VAR_NAME_PATTERN.test(envName)) {
    return '环境变量名仅支持字母/数字/下划线，且不能以数字开头。'
  }
  return null
}

interface ProviderApiKeyFieldProps {
  value: ProviderApiKeyInputValue
  onChange: (value: ProviderApiKeyInputValue) => void
  disabled?: boolean
  className?: string
  label?: string
  error?: string | null
}

export function ProviderApiKeyField({
  value,
  onChange,
  disabled,
  className,
  label = 'API Key',
  error,
}: ProviderApiKeyFieldProps) {
  const [showRawKey, setShowRawKey] = useState(false)

  return (
    <div className={cn('space-y-2', className)}>
      <Label className='text-xs uppercase tracking-[0.18em] text-muted-foreground'>{label}</Label>

      <Tabs
        className='space-y-2'
        onValueChange={(nextValue) => {
          const nextMode: ProviderApiKeyMode = nextValue === 'env' ? 'env' : 'direct'
          if (nextMode === value.mode) return
          onChange({
            mode: nextMode,
            value: '',
          })
        }}
        value={value.mode}
      >
        <TabsList className='grid w-full grid-cols-2'>
          <TabsTrigger value='direct'>
            <KeyRound className='h-3.5 w-3.5' />
            直接填
          </TabsTrigger>
          <TabsTrigger value='env'>
            <Variable className='h-3.5 w-3.5' />
            用变量
          </TabsTrigger>
        </TabsList>

        <TabsContent className='space-y-2' value='direct'>
          <div className='relative'>
            <Input
              className='pr-9'
              disabled={disabled}
              onChange={(event) =>
                onChange({
                  mode: 'direct',
                  value: event.target.value,
                })
              }
              placeholder='粘贴 API Key'
              type={showRawKey ? 'text' : 'password'}
              value={value.mode === 'direct' ? value.value : ''}
            />
            <Button
              aria-label={showRawKey ? '隐藏 API Key' : '显示 API Key'}
              className='absolute right-1 top-1/2 -translate-y-1/2'
              disabled={disabled}
              onClick={() => setShowRawKey((current) => !current)}
              size='icon-xs'
              type='button'
              variant='ghost'
            >
              {showRawKey ? <EyeOff className='h-3.5 w-3.5' /> : <Eye className='h-3.5 w-3.5' />}
            </Button>
          </div>
        </TabsContent>

        <TabsContent className='space-y-2' value='env'>
          <Input
            autoCapitalize='off'
            autoComplete='off'
            autoCorrect='off'
            disabled={disabled}
            onChange={(event) =>
              onChange({
                mode: 'env',
                value: event.target.value,
              })
            }
            placeholder='MINIMAX_API_KEY'
            spellCheck={false}
            value={value.mode === 'env' ? value.value : ''}
          />
          <p className='text-xs text-muted-foreground'>只填变量名，例如 `MINIMAX_API_KEY`。</p>
        </TabsContent>
      </Tabs>

      {error ? <p className='text-xs text-destructive'>{error}</p> : null}
    </div>
  )
}
