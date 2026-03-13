import { Bot, ChartColumnIncreasing, Palette, Server, SlidersHorizontal, type LucideIcon } from 'lucide-react'

import type { SettingsSection } from '@/store/settings-store'

export interface SettingsSectionMeta {
  label: string
  description: string
  icon: LucideIcon
}

export const DEFAULT_SETTINGS_SECTION: SettingsSection = 'theme'

export const settingsSections: SettingsSection[] = [
  'general',
  'theme',
  'memory',
  'providers',
  'usage',
]

export const settingsSectionMeta: Record<SettingsSection, SettingsSectionMeta> = {
  theme: {
    label: '主题',
    description: '管理界面模式、配色方案与字体显示',
    icon: Palette,
  },
  general: {
    label: '通用设置',
    description: '管理 Agent 基础配置与行为选项',
    icon: SlidersHorizontal,
  },
  memory: {
    label: '记忆文件',
    description: '编辑 MEMORY/PROFILE 与 memory/*.md',
    icon: Bot,
  },
  providers: {
    label: '模型服务商',
    description: '创建和编辑 OpenAI 兼容的服务商配置',
    icon: Server,
  },
  usage: {
    label: '使用统计',
    description: '查看 Turn、Token 消耗与工具调用统计',
    icon: ChartColumnIncreasing,
  },
}

export function isSettingsSection(value: string | null | undefined): value is SettingsSection {
  return (
    value === 'general' ||
    value === 'theme' ||
    value === 'memory' ||
    value === 'providers' ||
    value === 'usage'
  )
}

export function normalizeSettingsSection(
  value: string | null | undefined,
  fallback: SettingsSection = DEFAULT_SETTINGS_SECTION,
): SettingsSection {
  return isSettingsSection(value) ? value : fallback
}
