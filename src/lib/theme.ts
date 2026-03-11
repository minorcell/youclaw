import type { CustomThemePalette, ThemeFontSize, ThemeMode } from '@/store/settings-store'

const FONT_SIZE_CSS_VALUE: Record<ThemeFontSize, string> = {
  small: '15px',
  medium: '16px',
  large: '17px',
}

const SANS_FONT_STACK =
  "'Geist Variable', -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', sans-serif"
const SERIF_FONT_STACK =
  "'Noto Serif CJK SC', 'Source Han Serif SC', 'Songti SC', 'STSong', 'Times New Roman', serif"

const CUSTOM_THEME_VARS = [
  '--background',
  '--foreground',
  '--card',
  '--card-foreground',
  '--popover',
  '--popover-foreground',
  '--primary',
  '--primary-foreground',
  '--secondary',
  '--secondary-foreground',
  '--muted',
  '--muted-foreground',
  '--accent',
  '--accent-foreground',
  '--border',
  '--input',
  '--ring',
  '--sidebar',
  '--sidebar-foreground',
  '--sidebar-primary',
  '--sidebar-primary-foreground',
  '--sidebar-accent',
  '--sidebar-accent-foreground',
  '--sidebar-border',
  '--sidebar-ring',
] as const

function hexToRgb(hex: string): { r: number; g: number; b: number } | null {
  const cleaned = hex.trim().toLowerCase()
  const fullHex = /^#[0-9a-f]{6}$/
  if (!fullHex.test(cleaned)) return null
  return {
    r: Number.parseInt(cleaned.slice(1, 3), 16),
    g: Number.parseInt(cleaned.slice(3, 5), 16),
    b: Number.parseInt(cleaned.slice(5, 7), 16),
  }
}

function contrastColor(background: string): string {
  const rgb = hexToRgb(background)
  if (!rgb) return '#f5f5f5'
  const brightness = (rgb.r * 299 + rgb.g * 587 + rgb.b * 114) / 1000
  return brightness > 145 ? '#161616' : '#f5f5f5'
}

function applyCustomTheme(palette: CustomThemePalette) {
  const root = document.documentElement
  const foreground = palette.foreground
  const primaryForeground = contrastColor(palette.primary)
  const cardForeground = contrastColor(palette.card)
  const accentForeground = contrastColor(palette.accent)
  const sidebarForeground = contrastColor(palette.sidebar)

  root.style.setProperty('--background', palette.background)
  root.style.setProperty('--foreground', foreground)
  root.style.setProperty('--card', palette.card)
  root.style.setProperty('--card-foreground', cardForeground)
  root.style.setProperty('--popover', palette.card)
  root.style.setProperty('--popover-foreground', cardForeground)
  root.style.setProperty('--primary', palette.primary)
  root.style.setProperty('--primary-foreground', primaryForeground)
  root.style.setProperty('--secondary', palette.muted)
  root.style.setProperty('--secondary-foreground', foreground)
  root.style.setProperty('--muted', palette.muted)
  root.style.setProperty('--muted-foreground', foreground)
  root.style.setProperty('--accent', palette.accent)
  root.style.setProperty('--accent-foreground', accentForeground)
  root.style.setProperty('--border', palette.border)
  root.style.setProperty('--input', palette.border)
  root.style.setProperty('--ring', palette.primary)

  root.style.setProperty('--sidebar', palette.sidebar)
  root.style.setProperty('--sidebar-foreground', sidebarForeground)
  root.style.setProperty('--sidebar-primary', palette.primary)
  root.style.setProperty('--sidebar-primary-foreground', primaryForeground)
  root.style.setProperty('--sidebar-accent', palette.accent)
  root.style.setProperty('--sidebar-accent-foreground', accentForeground)
  root.style.setProperty('--sidebar-border', palette.border)
  root.style.setProperty('--sidebar-ring', palette.primary)
}

function clearCustomTheme() {
  const root = document.documentElement
  for (const cssVar of CUSTOM_THEME_VARS) {
    root.style.removeProperty(cssVar)
  }
}

function applyTypography(fontSize: ThemeFontSize, useSerif: boolean) {
  const root = document.documentElement
  root.style.setProperty('--app-font-size', FONT_SIZE_CSS_VALUE[fontSize])
  root.style.setProperty('--app-font-family', useSerif ? SERIF_FONT_STACK : SANS_FONT_STACK)
}

export function applyTheme(
  mode: ThemeMode,
  customTheme: CustomThemePalette,
  fontSize: ThemeFontSize,
  useSerif: boolean,
) {
  const root = document.documentElement
  applyTypography(fontSize, useSerif)
  clearCustomTheme()
  root.classList.remove('dark')

  if (mode === 'black') {
    root.classList.add('dark')
    root.style.colorScheme = 'dark'
    return
  }

  root.style.colorScheme = 'light'
  if (mode === 'custom') {
    applyCustomTheme(customTheme)
  }
}
