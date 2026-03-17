import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react'
import { GripVertical } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Navigate, Outlet, useLocation, useNavigate, useParams } from 'react-router-dom'

import { AppSidebar } from '@/components/layouts/sidebar'
import { DEFAULT_SETTINGS_SECTION, normalizeSettingsSection } from '@/pages/settings/sections'
import { getAppClient } from '@/lib/app-client'
import { flattenProviderProfiles } from '@/lib/provider-profiles'
import { cn } from '@/lib/utils'
import { LoadingScreen } from '@/pages/welcome'
import { useAppStore } from '@/store/app-store'
import type { SettingsSection } from '@/store/settings-store'

const SIDEBAR_MIN_WIDTH = 260
const SIDEBAR_MAX_WIDTH = 520
const SIDEBAR_DEFAULT_WIDTH = SIDEBAR_MIN_WIDTH
const CONTENT_MIN_WIDTH = 560
const SIDEBAR_RESIZE_STEP = 24
const SIDEBAR_HANDLE_HITBOX = 16
function clampValue(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

function isMacPlatform(): boolean {
  if (typeof navigator === 'undefined') {
    return false
  }
  const userAgentData = (navigator as Navigator & { userAgentData?: { platform?: string } })
    .userAgentData
  const platform = userAgentData?.platform ?? navigator.platform ?? ''
  return /mac/i.test(platform)
}

function settingsSectionFromPath(pathname: string): string | null {
  const segments = pathname.split('/').filter(Boolean)
  if (segments[0] !== 'settings') {
    return null
  }
  return segments[1] ?? null
}

export function AppLayout() {
  const navigate = useNavigate()
  const location = useLocation()
  const params = useParams<{ sessionId?: string }>()
  const shellRef = useRef<HTMLDivElement | null>(null)
  const resizeStartRef = useRef<{ startX: number; startWidth: number } | null>(null)
  const resizeAnimationFrameRef = useRef<number | null>(null)
  const pendingSidebarWidthRef = useRef<number | null>(null)
  const createSessionRequestRef = useRef<Promise<string> | null>(null)

  const sessionIdFromRoute = params.sessionId ?? null
  const isSettingsPage =
    location.pathname === '/settings' || location.pathname.startsWith('/settings/')
  const activeSettingsSection: SettingsSection | null = isSettingsPage
    ? normalizeSettingsSection(settingsSectionFromPath(location.pathname), DEFAULT_SETTINGS_SECTION)
    : null

  const initialized = useAppStore((state) => state.initialized)
  const providerAccounts = useAppStore((state) => state.providerAccounts)
  const sessions = useAppStore((state) => state.sessions)
  const messagesBySession = useAppStore((state) => state.messagesBySession)
  const activeSessionId = useAppStore((state) => state.activeSessionId)
  const [sidebarWidth, setSidebarWidth] = useState(SIDEBAR_DEFAULT_WIDTH)
  const [isResizingSidebar, setIsResizingSidebar] = useState(false)
  const providers = useMemo(() => flattenProviderProfiles(providerAccounts), [providerAccounts])

  const activeSession = useMemo(
    () => sessions.find((session) => session.id === sessionIdFromRoute) ?? null,
    [sessions, sessionIdFromRoute],
  )

  const selectedSidebarSessionId = sessionIdFromRoute ?? activeSessionId ?? sessions[0]?.id ?? null
  const contentRouteKey = sessionIdFromRoute ? `chat:${sessionIdFromRoute}` : location.pathname
  const useMacOverlayTitlebar = isMacPlatform()
  const latestSession = sessions[0] ?? null
  const latestSessionHasMessages = latestSession
    ? (messagesBySession[latestSession.id]?.length ?? 0) > 0
    : false
  const latestReusableSessionId =
    latestSession !== null && latestSession.last_turn_at === null && !latestSessionHasMessages
      ? latestSession.id
      : null

  function getSidebarMaxWidth() {
    const shellWidth = shellRef.current?.clientWidth
    if (!shellWidth) {
      return SIDEBAR_MAX_WIDTH
    }
    const maxByContainer = shellWidth - CONTENT_MIN_WIDTH
    return Math.max(SIDEBAR_MIN_WIDTH, Math.min(SIDEBAR_MAX_WIDTH, maxByContainer))
  }

  function clampSidebarWidth(width: number) {
    return clampValue(width, SIDEBAR_MIN_WIDTH, getSidebarMaxWidth())
  }

  function setSidebarWidthIfNeeded(nextWidth: number) {
    setSidebarWidth((currentWidth) => (currentWidth === nextWidth ? currentWidth : nextWidth))
  }

  function flushSidebarResizeFrame() {
    if (resizeAnimationFrameRef.current !== null) {
      cancelAnimationFrame(resizeAnimationFrameRef.current)
      resizeAnimationFrameRef.current = null
    }
    if (pendingSidebarWidthRef.current !== null) {
      setSidebarWidthIfNeeded(pendingSidebarWidthRef.current)
      pendingSidebarWidthRef.current = null
    }
  }

  function scheduleSidebarResizeWidth(nextWidth: number) {
    pendingSidebarWidthRef.current = nextWidth
    if (resizeAnimationFrameRef.current !== null) {
      return
    }
    resizeAnimationFrameRef.current = requestAnimationFrame(() => {
      resizeAnimationFrameRef.current = null
      if (pendingSidebarWidthRef.current === null) {
        return
      }
      setSidebarWidthIfNeeded(pendingSidebarWidthRef.current)
      pendingSidebarWidthRef.current = null
    })
  }

  useEffect(() => {
    function syncSidebarWidthToViewport() {
      setSidebarWidth((currentWidth) => clampSidebarWidth(currentWidth))
    }

    syncSidebarWidthToViewport()
    window.addEventListener('resize', syncSidebarWidthToViewport)
    return () => {
      window.removeEventListener('resize', syncSidebarWidthToViewport)
    }
  }, [])

  useEffect(() => {
    return () => {
      flushSidebarResizeFrame()
    }
  }, [])

  useEffect(() => {
    if (!isResizingSidebar) return
    const currentUserSelect = document.body.style.userSelect
    const currentCursor = document.body.style.cursor
    document.body.style.userSelect = 'none'
    document.body.style.cursor = 'col-resize'
    return () => {
      document.body.style.userSelect = currentUserSelect
      document.body.style.cursor = currentCursor
    }
  }, [isResizingSidebar])

  if (!initialized) {
    return <LoadingScreen />
  }

  if (providers.length === 0) {
    return <Navigate replace to='/welcome/provider' />
  }

  function buildChatPath(targetSessionId: string): string {
    return `/chat/${targetSessionId}`
  }

  function handleOpenChat() {
    const targetSessionId = sessionIdFromRoute ?? activeSessionId ?? sessions[0]?.id ?? null
    navigate({
      pathname: targetSessionId ? buildChatPath(targetSessionId) : '/',
    })
  }

  async function handleCreateSession() {
    if (latestReusableSessionId) {
      navigate({
        pathname: buildChatPath(latestReusableSessionId),
      })
      return
    }

    if (createSessionRequestRef.current) {
      const sessionId = await createSessionRequestRef.current
      navigate({
        pathname: buildChatPath(sessionId),
      })
      return
    }

    const fallbackProvider = providers[0] ?? null
    const providerForNewSession = activeSession?.provider_profile_id
      ? (providers.find((item) => item.id === activeSession.provider_profile_id) ??
        fallbackProvider)
      : fallbackProvider

    const createSessionRequest = (async () => {
      const created = await getAppClient().request<{ id: string }>('sessions.create', {
        provider_profile_id: providerForNewSession?.id ?? null,
      })
      return created.id
    })()
    createSessionRequestRef.current = createSessionRequest

    try {
      const sessionId = await createSessionRequest
      navigate({
        pathname: buildChatPath(sessionId),
      })
    } finally {
      if (createSessionRequestRef.current === createSessionRequest) {
        createSessionRequestRef.current = null
      }
    }
  }

  async function handleDeleteSession(targetSessionId: string) {
    await getAppClient().request('sessions.delete', {
      session_id: targetSessionId,
    })

    if (targetSessionId === sessionIdFromRoute) {
      navigate({ pathname: '/' })
    }
  }

  async function handleRenameSession(targetSessionId: string, title: string) {
    await getAppClient().request('sessions.rename', {
      session_id: targetSessionId,
      title,
    })
  }

  function handleSelectSession(targetSessionId: string) {
    navigate({
      pathname: buildChatPath(targetSessionId),
    })
  }

  function handleSidebarResizePointerDown(event: PointerEvent<HTMLButtonElement>) {
    resizeStartRef.current = {
      startX: event.clientX,
      startWidth: sidebarWidth,
    }
    setIsResizingSidebar(true)
    event.currentTarget.setPointerCapture(event.pointerId)
  }

  function handleSidebarResizePointerMove(event: PointerEvent<HTMLButtonElement>) {
    const resizeStart = resizeStartRef.current
    if (!resizeStart) {
      return
    }
    const deltaX = event.clientX - resizeStart.startX
    const nextWidth = clampSidebarWidth(resizeStart.startWidth + deltaX)
    scheduleSidebarResizeWidth(nextWidth)
  }

  function stopSidebarResize(event: PointerEvent<HTMLButtonElement>) {
    if (resizeStartRef.current === null) return
    flushSidebarResizeFrame()
    resizeStartRef.current = null
    setIsResizingSidebar(false)
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
  }

  function handleSidebarResizeKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') {
      return
    }
    event.preventDefault()
    const delta = event.key === 'ArrowRight' ? SIDEBAR_RESIZE_STEP : -SIDEBAR_RESIZE_STEP
    setSidebarWidth((currentWidth) => clampSidebarWidth(currentWidth + delta))
  }

  function handleWindowDragPointerDown(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) {
      return
    }
    // Keep native drag-region behavior while adding an explicit fallback.
    void getCurrentWindow().startDragging().catch(() => undefined)
  }
  const handlePosition = sidebarWidth

  return (
    <main className='box-border h-dvh select-none overflow-hidden bg-layout'>
      <div
        className='relative grid h-full min-h-0 overflow-hidden bg-background/65'
        ref={shellRef}
        style={{
          gridTemplateColumns: `${sidebarWidth}px minmax(0, 1fr)`,
        }}
      >
        <div className='min-h-0 overflow-hidden'>
          <AppSidebar
            activeSessionId={selectedSidebarSessionId}
            activeSettingsSection={activeSettingsSection}
            activeView={isSettingsPage ? 'settings' : 'chat'}
            enableWindowDrag={useMacOverlayTitlebar}
            onCreateSession={() => void handleCreateSession()}
            onDeleteSession={handleDeleteSession}
            onOpenChat={handleOpenChat}
            onOpenSettings={() =>
              navigate({
                pathname: `/settings/${DEFAULT_SETTINGS_SECTION}`,
              })
            }
            onWindowDragPointerDown={handleWindowDragPointerDown}
            onRenameSession={handleRenameSession}
            onSelectSession={handleSelectSession}
            onSelectSettingsSection={(section) =>
              navigate({
                pathname: `/settings/${section}`,
              })
            }
            providers={providers}
            sessions={sessions}
          />
        </div>

        <section
          className='h-full min-h-0 select-none overflow-hidden rounded-l-xl bg-background/85'
          key={contentRouteKey}
        >
          <Outlet />
        </section>

        <button
          aria-label='调整侧边栏宽度'
          aria-orientation='vertical'
          className={cn(
            'group absolute top-1/2 z-20 flex h-10 w-4 -translate-x-1/2 -translate-y-1/2 cursor-col-resize items-center justify-center rounded-full bg-background/85 text-muted-foreground shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50',
            isResizingSidebar
              ? 'bg-accent/50 text-foreground'
              : 'hover:bg-card hover:text-foreground',
          )}
          onKeyDown={handleSidebarResizeKeyDown}
          onPointerCancel={stopSidebarResize}
          onPointerDown={handleSidebarResizePointerDown}
          onPointerMove={handleSidebarResizePointerMove}
          onPointerUp={stopSidebarResize}
          style={{
            left: `${handlePosition}px`,
            width: `${SIDEBAR_HANDLE_HITBOX}px`,
          }}
          type='button'
        >
          <GripVertical className='h-3.5 w-3.5' />
        </button>
      </div>
    </main>
  )
}
