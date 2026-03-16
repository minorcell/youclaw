import { Navigate, Outlet, useParams } from 'react-router-dom'

import { ScrollArea } from '@/components/ui/scroll-area'
import { DEFAULT_SETTINGS_SECTION, isSettingsSection } from '@/pages/settings/sections'

export function SettingsLayoutPage() {
  const params = useParams<{ settingsSection?: string }>()
  const rawSection = params.settingsSection ?? null

  if (rawSection !== null && rawSection.length > 0 && !isSettingsSection(rawSection)) {
    return <Navigate replace to={`/settings/${DEFAULT_SETTINGS_SECTION}`} />
  }

  return (
    <div className='flex h-full min-h-0 flex-col select-none bg-background/70'>
      <ScrollArea className='min-h-0 flex-1' hideScrollbar>
        <div className='px-3 py-3 sm:px-4 sm:py-4'>
          <Outlet />
        </div>
      </ScrollArea>
    </div>
  )
}
