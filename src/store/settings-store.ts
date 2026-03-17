import { create } from 'zustand'
import { persist } from 'zustand/middleware'

export type ThemeMode = 'white' | 'black' | 'custom'
export type ThemePresetId = 'grass-green' | 'desert-yellow' | 'college-blue' | 'deep-sea-blue'
export type ThemeFontSize = 'small' | 'medium' | 'large'
export type SettingsSection = 'general' | 'theme' | 'memory' | 'providers' | 'archive' | 'usage'
export type SelectedProviderId = string | 'new'

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
    id: 'grass-green',
    label: '初春绿',
    description: '生机盎然的春芽绿，清新明快且富有活力。',
    palette: {
      background: '#eef9ec',
      foreground: '#19301f',
      card: '#f8fdf6',
      primary: '#39a94c',
      border: '#bfe0c3',
      muted: '#dcf0de',
      accent: '#cdeacb',
      sidebar: '#d6efd4',
    },
  },
  {
    id: 'desert-yellow',
    label: '沙漠黄',
    description: '羊皮卷暖黄，柔和复古并带有书卷感。',
    palette: {
      background: '#f6ecd8',
      foreground: '#3c2d1d',
      card: '#fdf5e7',
      primary: '#b48547',
      border: '#e1cca8',
      muted: '#f0e0c3',
      accent: '#e8d5b1',
      sidebar: '#ecddc0',
    },
  },
  {
    id: 'college-blue',
    label: '大学蓝',
    description: '晴空亮蓝，清透明快且保持阅读舒适。',
    palette: {
      background: '#eaf5ff',
      foreground: '#103356',
      card: '#f6fbff',
      primary: '#2a88e6',
      border: '#bfdaf2',
      muted: '#dbeeff',
      accent: '#cde6fb',
      sidebar: '#d4ebff',
    },
  },
  {
    id: 'deep-sea-blue',
    label: '深海蓝',
    description: '深海暗蓝，适合夜间沉浸与长时间专注。',
    palette: {
      background: '#0a1321',
      foreground: '#e3edff',
      card: '#111d2f',
      primary: '#2f76c0',
      border: '#24364d',
      muted: '#16243a',
      accent: '#1d304a',
      sidebar: '#0d1b2d',
    },
  },
]

export const defaultThemePresetId: ThemePresetId = 'grass-green'

const themePresetPaletteMap: Record<ThemePresetId, CustomThemePalette> = themePresets.reduce(
  (accumulator, preset) => {
    accumulator[preset.id] = preset.palette
    return accumulator
  },
  {} as Record<ThemePresetId, CustomThemePalette>,
)

function getPresetPalette(presetId: ThemePresetId): CustomThemePalette {
  return themePresetPaletteMap[presetId]
}

interface PersistedSettingsThemeState {
  mode: ThemeMode
  preset: ThemePresetId
  custom: CustomThemePalette
  fontSize: ThemeFontSize
  useSerif: boolean
}

interface SettingsStoreState extends PersistedSettingsThemeState {
  selectedProviderId: SelectedProviderId
  setMode: (mode: ThemeMode) => void
  setPreset: (preset: ThemePresetId) => void
  setFontSize: (fontSize: ThemeFontSize) => void
  setUseSerif: (useSerif: boolean) => void
  resetCustomTheme: () => void
  setSelectedProviderId: (providerId: SelectedProviderId) => void
  resetSettingsUiState: () => void
}

const defaultSettingsUiState = {
  selectedProviderId: 'new' as SelectedProviderId,
}

export const useSettingsStore = create<SettingsStoreState>()(
  persist(
    (set) => ({
      mode: 'white',
      preset: defaultThemePresetId,
      custom: getPresetPalette(defaultThemePresetId),
      fontSize: 'medium',
      useSerif: false,
      ...defaultSettingsUiState,
      setMode: (mode) => set({ mode }),
      setPreset: (preset) =>
        set({
          preset,
          custom: getPresetPalette(preset),
        }),
      setFontSize: (fontSize) => set({ fontSize }),
      setUseSerif: (useSerif) => set({ useSerif }),
      resetCustomTheme: () =>
        set({
          preset: defaultThemePresetId,
          custom: getPresetPalette(defaultThemePresetId),
        }),
      setSelectedProviderId: (selectedProviderId) => set({ selectedProviderId }),
      resetSettingsUiState: () => set(defaultSettingsUiState),
    }),
    {
      name: 'youclaw.theme',
      version: 5,
      partialize: (state) => ({
        mode: state.mode,
        preset: state.preset,
        custom: state.custom,
        fontSize: state.fontSize,
        useSerif: state.useSerif,
      }),
    },
  ),
)
