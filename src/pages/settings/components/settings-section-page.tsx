import { Navigate, useParams } from 'react-router-dom'

import { AgentConfigSettingsSection } from '@/pages/settings/pages/general/components/agent-config-settings-section'
import { DesktopIntegrationSettingsSection } from '@/pages/settings/pages/general/components/desktop-integration-settings-section'
import { ArchiveSettingsPage } from '@/pages/settings/pages/archive'
import { AgentMemoryFilesSection } from '@/pages/settings/pages/memory/components/agent-memory-files-section'
import { ProvidersSettingsPage } from '@/pages/settings/pages/providers'
import { ThemeSettingsSection } from '@/pages/settings/pages/theme/components/theme-settings-section'
import { UsageSettingsSection } from '@/pages/settings/pages/usage/components/usage-settings-section'
import { DEFAULT_SETTINGS_SECTION, isSettingsSection } from '@/pages/settings/sections'
import { useAppStore } from '@/store/app-store'
import { useSettingsStore } from '@/store/settings-store'

export function SettingsSectionPage() {
  const params = useParams<{ settingsSection?: string }>()
  const rawSection = params.settingsSection ?? null

  if (!isSettingsSection(rawSection)) {
    return <Navigate replace to={`/settings/${DEFAULT_SETTINGS_SECTION}`} />
  }

  const providerAccounts = useAppStore((state) => state.providerAccounts)
  const mode = useSettingsStore((state) => state.mode)
  const preset = useSettingsStore((state) => state.preset)
  const fontSize = useSettingsStore((state) => state.fontSize)
  const useSerif = useSettingsStore((state) => state.useSerif)
  const setMode = useSettingsStore((state) => state.setMode)
  const setPreset = useSettingsStore((state) => state.setPreset)
  const setFontSize = useSettingsStore((state) => state.setFontSize)
  const setUseSerif = useSettingsStore((state) => state.setUseSerif)

  function handleThemeModeChange(value: string | null) {
    if (!value) return
    if (value === 'white' || value === 'black' || value === 'custom') {
      setMode(value)
    }
  }

  if (rawSection === 'theme') {
    return (
      <ThemeSettingsSection
        fontSize={fontSize}
        mode={mode}
        onFontSizeChange={setFontSize}
        onModeChange={handleThemeModeChange}
        onPresetChange={setPreset}
        onUseSerifChange={setUseSerif}
        preset={preset}
        useSerif={useSerif}
      />
    )
  }

  if (rawSection === 'general') {
    return (
      <div className='space-y-4'>
        <DesktopIntegrationSettingsSection />
        <AgentConfigSettingsSection />
      </div>
    )
  }

  if (rawSection === 'memory') {
    return <AgentMemoryFilesSection />
  }

  if (rawSection === 'providers') {
    return <ProvidersSettingsPage />
  }

  if (rawSection === 'archive') {
    return <ArchiveSettingsPage />
  }

  return <UsageSettingsSection providerAccounts={providerAccounts} />
}
