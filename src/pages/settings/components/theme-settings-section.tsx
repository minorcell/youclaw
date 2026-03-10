import { MoonStar, Palette, Sun } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Card, CardContent } from "@/components/ui/card"
import { Label } from "@/components/ui/label"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  type ThemeMode,
  type ThemePresetId,
  themePresets,
} from "@/store/theme-store"
import { cn } from "@/lib/utils"

interface ThemeSettingsSectionProps {
  mode: ThemeMode
  preset: ThemePresetId
  onModeChange: (value: string | null) => void
  onPresetChange: (preset: ThemePresetId) => void
}

const modeLabel: Record<ThemeMode, string> = {
  white: "White",
  black: "Black",
  custom: "Custom",
}

export function ThemeSettingsSection({
  mode,
  preset,
  onModeChange,
  onPresetChange,
}: ThemeSettingsSectionProps) {
  const selectedPreset =
    themePresets.find((item) => item.id === preset) ?? themePresets[0]

  return (
    <div className="space-y-4">
      <Card className="border-border/70 bg-background/80 py-0 shadow-none">
        <CardContent className="grid gap-4 py-4 md:grid-cols-[220px_minmax(0,1fr)]">
          <div className="space-y-2">
            <Label className="text-xs uppercase tracking-[0.16em] text-muted-foreground">
              Theme Mode
            </Label>
            <Select onValueChange={onModeChange} value={mode}>
              <SelectTrigger className="h-10 w-full rounded-xl">
                <SelectValue placeholder="Select mode" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="white">White</SelectItem>
                <SelectItem value="black">Black</SelectItem>
                <SelectItem value="custom">Custom</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="rounded-xl border border-border bg-card px-3 py-2">
            <p className="text-xs uppercase tracking-[0.16em] text-muted-foreground">
              当前模式
            </p>
            <div className="mt-2 flex items-center gap-2">
              <Badge className="gap-1 border-border bg-background text-foreground">
                {mode === "white" ? (
                  <Sun className="h-3 w-3" />
                ) : mode === "black" ? (
                  <MoonStar className="h-3 w-3" />
                ) : (
                  <Palette className="h-3 w-3" />
                )}
                {modeLabel[mode]}
              </Badge>
              {mode === "custom" ? (
                <span className="text-xs text-muted-foreground">
                  {selectedPreset.label}
                </span>
              ) : null}
            </div>
          </div>
        </CardContent>
      </Card>

      {mode === "custom" ? (
        <Card className="border-border/70 bg-background/80 py-0 shadow-none">
          <CardContent className="space-y-4 py-4">
            <div>
              <p className="text-sm font-medium">预设主题配色</p>
              <p className="text-sm text-muted-foreground">
                使用系统内置配色，避免手动调色负担。
              </p>
            </div>

            <RadioGroup
              className="grid gap-3 md:grid-cols-3"
              onValueChange={(value) => onPresetChange(value as ThemePresetId)}
              value={preset}
            >
              {themePresets.map((item) => (
                <Label
                  className="block cursor-pointer"
                  htmlFor={`theme-preset-${item.id}`}
                  key={item.id}
                >
                  <Card
                    className={cn(
                      "border py-0 shadow-none transition-colors",
                      preset === item.id
                        ? "border-primary/60 bg-accent/40"
                        : "border-border bg-card/80 hover:bg-accent/25",
                    )}
                  >
                    <CardContent className="space-y-3 py-4">
                      <div className="flex items-start justify-between gap-3">
                        <div>
                          <p className="font-medium">{item.label}</p>
                          <p className="mt-1 text-xs text-muted-foreground">
                            {item.description}
                          </p>
                        </div>
                        <RadioGroupItem
                          id={`theme-preset-${item.id}`}
                          value={item.id}
                        />
                      </div>

                      <div className="flex items-center gap-2">
                        <PresetSwatch color={item.palette.background} />
                        <PresetSwatch color={item.palette.card} />
                        <PresetSwatch color={item.palette.primary} />
                        <PresetSwatch color={item.palette.accent} />
                      </div>
                    </CardContent>
                  </Card>
                </Label>
              ))}
            </RadioGroup>
          </CardContent>
        </Card>
      ) : null}
    </div>
  )
}

function PresetSwatch({ color }: { color: string }) {
  return (
    <span
      className="h-5 w-5 rounded-full border border-border/80"
      style={{ backgroundColor: color }}
    />
  )
}
