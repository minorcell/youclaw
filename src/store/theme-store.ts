import { create } from "zustand"
import { persist } from "zustand/middleware"

export type ThemeMode = "white" | "black" | "custom"
export type ThemePresetId = "ocean-blue" | "grass-green" | "desert-yellow"

export interface CustomThemePalette {
  background: string
  foreground: string
  card: string
  primary: string
  border: string
  muted: string
  accent: string
  sidebar: string
}

export interface ThemePreset {
  id: ThemePresetId
  label: string
  description: string
  palette: CustomThemePalette
}

export const themePresets: ThemePreset[] = [
  {
    id: "ocean-blue",
    label: "海洋蓝",
    description: "冷静清爽，适合长时间对话与阅读。",
    palette: {
      background: "#edf4fb",
      foreground: "#162031",
      card: "#f7fbff",
      primary: "#1f5fa6",
      border: "#c6d8ec",
      muted: "#dfeaf6",
      accent: "#cfe2f8",
      sidebar: "#d7e6f7",
    },
  },
  {
    id: "grass-green",
    label: "草坪绿",
    description: "自然柔和，强调稳定与专注感。",
    palette: {
      background: "#eef6ef",
      foreground: "#1d291f",
      card: "#f9fdf8",
      primary: "#2f7d3f",
      border: "#c7ddcb",
      muted: "#e0ede2",
      accent: "#d0e7d4",
      sidebar: "#d7eadb",
    },
  },
  {
    id: "desert-yellow",
    label: "沙漠黄",
    description: "暖色低刺激，适合夜间低压办公。",
    palette: {
      background: "#f9f4e9",
      foreground: "#2f2619",
      card: "#fffaf1",
      primary: "#b27a2d",
      border: "#e7d2ae",
      muted: "#f3e7d2",
      accent: "#ecd8b6",
      sidebar: "#efddbf",
    },
  },
]

export const defaultThemePresetId: ThemePresetId = "ocean-blue"

const themePresetPaletteMap: Record<ThemePresetId, CustomThemePalette> =
  themePresets.reduce(
    (accumulator, preset) => {
      accumulator[preset.id] = preset.palette
      return accumulator
    },
    {} as Record<ThemePresetId, CustomThemePalette>,
  )

function getPresetPalette(presetId: ThemePresetId): CustomThemePalette {
  return themePresetPaletteMap[presetId]
}

interface ThemeStoreState {
  mode: ThemeMode
  preset: ThemePresetId
  custom: CustomThemePalette
  setMode: (mode: ThemeMode) => void
  setPreset: (preset: ThemePresetId) => void
  resetCustomTheme: () => void
}

export const useThemeStore = create<ThemeStoreState>()(
  persist(
    (set) => ({
      mode: "white",
      preset: defaultThemePresetId,
      custom: getPresetPalette(defaultThemePresetId),
      setMode: (mode) => set({ mode }),
      setPreset: (preset) =>
        set({
          preset,
          custom: getPresetPalette(preset),
        }),
      resetCustomTheme: () =>
        set({
          preset: defaultThemePresetId,
          custom: getPresetPalette(defaultThemePresetId),
        }),
    }),
    {
      name: "baogongtou.theme",
      version: 2,
      migrate: (persistedState, version) => {
        const state = (persistedState ?? {}) as Partial<ThemeStoreState>
        if (version < 2) {
          const nextPreset = defaultThemePresetId
          return {
            mode: state.mode ?? "white",
            preset: nextPreset,
            custom: getPresetPalette(nextPreset),
          }
        }

        const nextPreset = state.preset ?? defaultThemePresetId
        return {
          mode: state.mode ?? "white",
          preset: nextPreset,
          custom: getPresetPalette(nextPreset),
        }
      },
    },
  ),
)
