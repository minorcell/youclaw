import { MoonStar, Palette, Sun, type LucideIcon } from 'lucide-react'

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Label } from '@/components/ui/label'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { Switch } from '@/components/ui/switch'
import {
  type ThemeFontSize,
  type ThemeMode,
  type ThemePresetId,
  themePresets,
} from '@/store/settings-store'
import { cn } from '@/lib/utils'

interface ThemeSettingsSectionProps {
  mode: ThemeMode
  preset: ThemePresetId
  fontSize: ThemeFontSize
  useSerif: boolean
  onModeChange: (value: string | null) => void
  onPresetChange: (preset: ThemePresetId) => void
  onFontSizeChange: (fontSize: ThemeFontSize) => void
  onUseSerifChange: (useSerif: boolean) => void
}

interface ModeOption {
  id: ThemeMode
  label: string
  description: string
  icon: LucideIcon
}

interface FontSizeOption {
  id: ThemeFontSize
  label: string
  sliderValue: number
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

const fontSizeOptions: FontSizeOption[] = [
  { id: 'small', label: '小', sliderValue: 0 },
  { id: 'medium', label: '中', sliderValue: 1 },
  { id: 'large', label: '大', sliderValue: 2 },
]

const FONT_SIZE_MIN = fontSizeOptions[0].sliderValue
const FONT_SIZE_MAX = fontSizeOptions[fontSizeOptions.length - 1].sliderValue
const MODE_CARD_BASE_CLASS = 'h-full min-h-32 rounded-xl p-3 transition-colors'
const PRESET_CARD_BASE_CLASS = 'h-full min-h-10 rounded-md px-2 py-1.5 transition-colors'

function getSelectableCardClass(isSelected: boolean, compact = false): string {
  return cn(
    compact ? PRESET_CARD_BASE_CLASS : MODE_CARD_BASE_CLASS,
    isSelected ? 'bg-accent/60' : 'bg-background/85 hover:bg-accent/25',
  )
}

export function ThemeSettingsSection({
  mode,
  preset,
  fontSize,
  useSerif,
  onModeChange,
  onPresetChange,
  onFontSizeChange,
  onUseSerifChange,
}: ThemeSettingsSectionProps) {
  const currentFontSize =
    fontSizeOptions.find((option) => option.id === fontSize) ?? fontSizeOptions[1]

  return (
    <Card className='bg-card/80 py-0 shadow-none'>
      <CardHeader className='py-4'>
        <CardTitle>界面模式</CardTitle>
        <CardDescription>
          选择系统主视觉模式。切换到自定义配色后，可在下方选择主题预设。
        </CardDescription>
      </CardHeader>
      <CardContent className='space-y-4 pb-2'>
        <RadioGroup
          className='grid auto-rows-fr gap-3 sm:grid-cols-3'
          onValueChange={(value) => onModeChange(value)}
          value={mode}
        >
          {modeOptions.map((item) => {
            const Icon = item.icon
            const inputId = `theme-mode-${item.id}`
            const isSelected = mode === item.id
            return (
              <Label className='block h-full cursor-pointer' htmlFor={inputId} key={item.id}>
                <div className={cn(getSelectableCardClass(isSelected), 'flex flex-col')}>
                  <div className='flex items-start justify-between gap-3'>
                    <span
                      className={cn(
                        'inline-flex size-8 items-center justify-center rounded-lg',
                        isSelected ? 'bg-background' : 'bg-background/70',
                      )}
                    >
                      <Icon className='h-4 w-4 text-muted-foreground' />
                    </span>
                    <RadioGroupItem id={inputId} value={item.id} />
                  </div>
                  <div className='mt-3'>
                    <p className='text-sm font-medium'>{item.label}</p>
                    <p className='mt-1 text-xs text-muted-foreground'>{item.description}</p>
                  </div>
                </div>
              </Label>
            )
          })}
        </RadioGroup>

        {mode === 'custom' ? (
          <RadioGroup
            className='grid auto-rows-fr gap-2 sm:grid-cols-4'
            onValueChange={(value) => onPresetChange(value as ThemePresetId)}
            value={preset}
          >
            {themePresets.map((item) => (
              <Label
                className='block h-full cursor-pointer'
                htmlFor={`theme-preset-${item.id}`}
                key={item.id}
              >
                <div
                  className={cn(
                    getSelectableCardClass(preset === item.id, true),
                    'flex items-center justify-between gap-2',
                  )}
                >
                  <div className='min-w-0 flex-1'>
                    <p className='truncate text-xs font-medium'>{item.label}</p>
                  </div>
                  <div className='flex items-center gap-1.5'>
                    <PresetSwatch color={item.palette.background} compact />
                    <PresetSwatch color={item.palette.card} compact />
                    <PresetSwatch color={item.palette.primary} compact />
                    <PresetSwatch color={item.palette.accent} compact />
                    <RadioGroupItem id={`theme-preset-${item.id}`} value={item.id} />
                  </div>
                </div>
              </Label>
            ))}
          </RadioGroup>
        ) : null}

        <div className='space-y-3 rounded-xl bg-background/85 p-3'>
          <div className='flex items-center justify-between gap-3'>
            <div>
              <p className='text-sm font-medium'>字体大小</p>
              <p className='text-xs text-muted-foreground'>通过滑块切换全局字号等级。</p>
            </div>
            <span className='rounded-full bg-card px-2 py-0.5 text-xs'>
              {currentFontSize.label}
            </span>
          </div>
          <input
            aria-label='字体大小'
            className='h-2 w-full cursor-pointer appearance-none rounded-full bg-muted accent-primary'
            max={FONT_SIZE_MAX}
            min={FONT_SIZE_MIN}
            onChange={(event) => {
              const sliderValue = Number.parseInt(event.target.value, 10)
              const next = fontSizeOptions.find((option) => option.sliderValue === sliderValue)
              if (next) {
                onFontSizeChange(next.id)
              }
            }}
            step={1}
            type='range'
            value={currentFontSize.sliderValue}
          />
          <div className='grid grid-cols-3 text-xs text-muted-foreground'>
            {fontSizeOptions.map((option) => (
              <span
                className={cn(
                  option.sliderValue === FONT_SIZE_MIN
                    ? 'text-left'
                    : option.sliderValue === FONT_SIZE_MAX
                      ? 'text-right'
                      : 'text-center',
                )}
                key={option.id}
              >
                {option.label}
              </span>
            ))}
          </div>
        </div>

        <div className='flex items-center justify-between rounded-xl bg-background/85 px-3 py-2.5'>
          <div>
            <p className='text-sm font-medium'>衬线字体</p>
            <p className='text-xs text-muted-foreground'>开启后使用 serif 风格作为全局主字体。</p>
          </div>
          <Switch checked={useSerif} onCheckedChange={onUseSerifChange} />
        </div>
      </CardContent>
    </Card>
  )
}

function PresetSwatch({ color, compact = false }: { color: string; compact?: boolean }) {
  return (
    <span
      className={cn('rounded-full', compact ? 'h-3 w-3' : 'h-4 w-4')}
      style={{ backgroundColor: color }}
    />
  )
}
