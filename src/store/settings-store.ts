import { create } from "zustand"
import { persist } from "zustand/middleware"

export type ThemeMode = "white" | "black" | "custom"
export type ThemePresetId = "grass-green" | "desert-yellow"
export type SettingsSection = "general" | "providers"
export type SelectedProviderId = string | "new"

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

export const defaultThemePresetId: ThemePresetId = "grass-green"

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

function isThemePresetId(value: unknown): value is ThemePresetId {
  return value === "grass-green" || value === "desert-yellow"
}

function normalizePresetId(value: unknown): ThemePresetId {
  if (isThemePresetId(value)) {
    return value
  }
  return defaultThemePresetId
}

interface PersistedSettingsThemeState {
  mode: ThemeMode
  preset: ThemePresetId
  custom: CustomThemePalette
}

interface SettingsStoreState extends PersistedSettingsThemeState {
  settingsSection: SettingsSection
  selectedProviderId: SelectedProviderId
  setMode: (mode: ThemeMode) => void
  setPreset: (preset: ThemePresetId) => void
  resetCustomTheme: () => void
  setSettingsSection: (section: SettingsSection) => void
  setSelectedProviderId: (providerId: SelectedProviderId) => void
  resetSettingsUiState: () => void
}

const defaultSettingsUiState = {
  settingsSection: "general" as SettingsSection,
  selectedProviderId: "new" as SelectedProviderId,
}

export const useSettingsStore = create<SettingsStoreState>()(
  persist(
    (set) => ({
      mode: "white",
      preset: defaultThemePresetId,
      custom: getPresetPalette(defaultThemePresetId),
      ...defaultSettingsUiState,
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
      setSettingsSection: (settingsSection) => set({ settingsSection }),
      setSelectedProviderId: (selectedProviderId) => set({ selectedProviderId }),
      resetSettingsUiState: () => set(defaultSettingsUiState),
    }),
    {
      name: "bgtclaw.theme",
      version: 4,
      partialize: (state) => ({
        mode: state.mode,
        preset: state.preset,
        custom: state.custom,
      }),
      migrate: (persistedState, _version) => {
        const state = (persistedState ?? {}) as Partial<
          PersistedSettingsThemeState & { preset?: string }
        >
        const nextPreset = normalizePresetId(state.preset)
        return {
          mode: state.mode ?? "white",
          preset: nextPreset,
          custom: getPresetPalette(nextPreset),
          ...defaultSettingsUiState,
        }
      },
    },
  ),
)
