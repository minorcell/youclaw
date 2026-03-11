import { MoonStar, Palette, Sun, type LucideIcon } from 'lucide-react'

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Label } from '@/components/ui/label'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { type ThemeMode, type ThemePresetId, themePresets } from '@/store/settings-store'
import { cn } from '@/lib/utils'

interface ThemeSettingsSectionProps {
  mode: ThemeMode
  preset: ThemePresetId
  onModeChange: (value: string | null) => void
  onPresetChange: (preset: ThemePresetId) => void
}

interface ModeOption {
  id: ThemeMode
  label: string
  description: string
  icon: LucideIcon
}

const modeOptions: ModeOption[] = [
  {
    id: 'white',
    label: '浅色模式',
    description: '明亮背景，适合白天与办公场景。',
    icon: Sun,
  },
  {
    id: 'black',
    label: '深色模式',
    description: '降低亮度刺激，适合夜间使用。',
    icon: MoonStar,
  },
  {
    id: 'custom',
    label: '自定义配色',
    description: '使用系统预设色板打造个性风格。',
    icon: Palette,
  },
]

export function ThemeSettingsSection({
  mode,
  preset,
  onModeChange,
  onPresetChange,
}: ThemeSettingsSectionProps) {
  return (
    <Card className='border border-border/70 bg-card/80 py-0 shadow-none'>
      <CardHeader className='py-4'>
        <CardTitle>界面模式</CardTitle>
        <CardDescription>
          选择系统主视觉模式。切换到自定义配色后，可在下方选择主题预设。
        </CardDescription>
      </CardHeader>
      <CardContent className='space-y-4 py-4'>
        <RadioGroup
          className='grid gap-3 sm:grid-cols-3'
          onValueChange={(value) => onModeChange(value)}
          value={mode}
        >
          {modeOptions.map((item) => {
            const Icon = item.icon
            const inputId = `theme-mode-${item.id}`
            return (
              <Label className='block cursor-pointer' htmlFor={inputId} key={item.id}>
                <div
                  className={cn(
                    'h-full rounded-xl border p-3 transition-colors',
                    mode === item.id
                      ? 'border-border bg-accent/45'
                      : 'border-border/70 bg-background/85 hover:bg-accent/20',
                  )}
                >
                  <div className='flex items-start justify-between gap-3'>
                    <span
                      className={cn(
                        'inline-flex size-8 items-center justify-center rounded-lg border',
                        mode === item.id
                          ? 'border-border bg-background'
                          : 'border-border/70 bg-background/70',
                      )}
                    >
                      <Icon className='h-4 w-4 text-muted-foreground' />
                    </span>
                    <RadioGroupItem id={inputId} value={item.id} />
                  </div>
                  <p className='mt-3 text-sm font-medium'>{item.label}</p>
                  <p className='mt-1 text-xs text-muted-foreground'>{item.description}</p>
                </div>
              </Label>
            )
          })}
        </RadioGroup>
      </CardContent>

      {mode === 'custom' ? (
        <>
          <div className='space-y-4 px-4 py-4'>
            <div>
              <p className='text-base font-medium'>预设主题配色</p>
              <p className='text-sm text-muted-foreground'>
                选择系统内置配色方案，快速统一全站视觉风格。
              </p>
            </div>
            <RadioGroup
              className='grid gap-3 xl:grid-cols-3'
              onValueChange={(value) => onPresetChange(value as ThemePresetId)}
              value={preset}
            >
              {themePresets.map((item) => (
                <Label
                  className='block cursor-pointer'
                  htmlFor={`theme-preset-${item.id}`}
                  key={item.id}
                >
                  <div
                    className={cn(
                      'h-full rounded-xl border p-3 transition-colors',
                      preset === item.id
                        ? 'border-border bg-accent/45'
                        : 'border-border/70 bg-background/85 hover:bg-accent/20',
                    )}
                  >
                    <div className='flex items-start justify-between gap-3'>
                      <div>
                        <p className='text-sm font-medium'>{item.label}</p>
                        <p className='mt-1 text-xs text-muted-foreground'>{item.description}</p>
                      </div>
                      <RadioGroupItem id={`theme-preset-${item.id}`} value={item.id} />
                    </div>

                    <div className='mt-3 flex items-center gap-2'>
                      <PresetSwatch color={item.palette.background} />
                      <PresetSwatch color={item.palette.card} />
                      <PresetSwatch color={item.palette.primary} />
                      <PresetSwatch color={item.palette.accent} />
                    </div>
                  </div>
                </Label>
              ))}
            </RadioGroup>
          </div>
        </>
      ) : null}
    </Card>
  )
}

function PresetSwatch({ color }: { color: string }) {
  return (
    <span
      className='h-5 w-5 rounded-full border border-border/80'
      style={{ backgroundColor: color }}
    />
  )
}
