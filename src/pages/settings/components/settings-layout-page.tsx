import { useMemo } from 'react'
import { Outlet, useLocation, useNavigate } from 'react-router-dom'

import { ScrollArea } from '@/components/ui/scroll-area'
import { cn } from '@/lib/utils'
import {
  DEFAULT_SETTINGS_SECTION,
  normalizeSettingsSection,
  settingsSectionMeta,
  settingsSections,
} from '@/pages/settings/sections'
import type { SettingsSection } from '@/store/settings-store'

function settingsSectionFromPath(pathname: string): string | null {
  const segments = pathname.split('/').filter(Boolean)
  if (segments[0] !== 'settings') {
    return null
  }
  return segments[1] ?? null
}

export function SettingsLayoutPage() {
  const navigate = useNavigate()
  const location = useLocation()

  const section = useMemo(
    () =>
      normalizeSettingsSection(
        settingsSectionFromPath(location.pathname),
        DEFAULT_SETTINGS_SECTION,
      ),
    [location.pathname],
  )

  function handleSectionChange(nextSection: SettingsSection) {
    if (nextSection === section) {
      return
    }
    navigate({ pathname: `/settings/${nextSection}` })
  }

  const sectionMeta = settingsSectionMeta[section]

  return (
    <div className='flex h-full min-h-0 flex-col select-none bg-card/80'>
      <header className='bg-card/80 px-4 py-2 sm:px-6'>
        <p className='text-xs uppercase tracking-[0.18em] text-muted-foreground'>设置</p>
        <p className='mt-1 text-sm text-muted-foreground'>{sectionMeta.description}</p>
      </header>

      <div className='grid min-h-0 flex-1 bg-background/70 md:grid-cols-[248px_minmax(0,1fr)]'>
        <aside className='hidden bg-muted/35 p-2.5 md:block'>
          <nav aria-label='设置导航' className='space-y-1.5'>
            {settingsSections.map((itemId) => {
              const item = settingsSectionMeta[itemId]
              const Icon = item.icon
              return (
                <button
                  aria-current={section === itemId ? 'page' : undefined}
                  className={cn(
                    'group w-full rounded-xl px-3 py-2 text-left transition-colors',
                    section === itemId
                      ? 'bg-accent/55 text-accent-foreground'
                      : 'bg-transparent text-muted-foreground hover:bg-accent/30 hover:text-foreground',
                  )}
                  key={itemId}
                  onClick={() => handleSectionChange(itemId)}
                  type='button'
                >
                  <div className='flex items-center gap-2'>
                    <Icon className='h-4 w-4' />
                    <span className='text-sm font-medium'>{item.label}</span>
                  </div>
                </button>
              )
            })}
          </nav>
        </aside>

        <section className='flex min-h-0 flex-1 flex-col'>
          <div className='p-2.5 md:hidden'>
            <div className='grid grid-cols-2 gap-1.5'>
              {settingsSections.map((itemId) => {
                const item = settingsSectionMeta[itemId]
                const Icon = item.icon
                return (
                  <button
                    className={cn(
                      'flex items-center justify-center gap-2 rounded-xl px-2 py-1.5 text-sm font-medium transition-colors',
                      section === itemId
                        ? 'bg-background text-foreground'
                        : 'bg-card/50 text-muted-foreground',
                    )}
                    key={itemId}
                    onClick={() => handleSectionChange(itemId)}
                    type='button'
                  >
                    <Icon className='h-4 w-4' />
                    {item.label}
                  </button>
                )
              })}
            </div>
          </div>

          <ScrollArea className='min-h-0 flex-1' hideScrollbar>
            <div className='px-3 py-3'>
              <Outlet />
            </div>
          </ScrollArea>
        </section>
      </div>
    </div>
  )
}
