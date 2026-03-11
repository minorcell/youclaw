import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent,
} from "react"
import { GripVertical } from "lucide-react"
import {
  Navigate,
  Outlet,
  useLocation,
  useNavigate,
  useParams,
  useSearchParams,
} from "react-router-dom"

import { SessionSidebar } from "@/components/layouts/sidebar"
import { SettingsModal } from "@/pages/settings"
import { getAppClient } from "@/lib/app-client"
import { flattenProviderProfiles } from "@/lib/provider-profiles"
import { cn } from "@/lib/utils"
import { LoadingScreen } from "@/pages/welcome"
import { useAppStore } from "@/store/app-store"

const SIDEBAR_DEFAULT_WIDTH = 330
const SIDEBAR_MIN_WIDTH = 260
const SIDEBAR_MAX_WIDTH = 520
const CONTENT_MIN_WIDTH = 560
const SIDEBAR_RESIZE_STEP = 24
const SIDEBAR_HANDLE_HITBOX = 16

function clampValue(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

export function AppLayout() {
  const navigate = useNavigate()
  const location = useLocation()
  const params = useParams<{ sessionId?: string }>()
  const [searchParams, setSearchParams] = useSearchParams()
  const shellRef = useRef<HTMLDivElement | null>(null)
  const resizeStartRef = useRef<{ startX: number; startWidth: number } | null>(
    null,
  )
  const resizeAnimationFrameRef = useRef<number | null>(null)
  const pendingSidebarWidthRef = useRef<number | null>(null)

  const sessionIdFromRoute = params.sessionId ?? null
  const isSettingsOpen = searchParams.get("settings") === "1"

  const initialized = useAppStore((state) => state.initialized)
  const providerAccounts = useAppStore((state) => state.providerAccounts)
  const sessions = useAppStore((state) => state.sessions)
  const activeSessionId = useAppStore((state) => state.activeSessionId)
  const [sidebarWidth, setSidebarWidth] = useState(SIDEBAR_DEFAULT_WIDTH)
  const [isResizingSidebar, setIsResizingSidebar] = useState(false)
  const providers = useMemo(
    () => flattenProviderProfiles(providerAccounts),
    [providerAccounts],
  )

  const activeSession = useMemo(
    () => sessions.find((session) => session.id === sessionIdFromRoute) ?? null,
    [sessions, sessionIdFromRoute],
  )

  const selectedSidebarSessionId =
    sessionIdFromRoute ?? activeSessionId ?? sessions[0]?.id ?? null

  function getSidebarMaxWidth() {
    const shellWidth = shellRef.current?.clientWidth
    if (!shellWidth) {
      return SIDEBAR_MAX_WIDTH
    }
    const maxByContainer = shellWidth - CONTENT_MIN_WIDTH
    return Math.max(
      SIDEBAR_MIN_WIDTH,
      Math.min(SIDEBAR_MAX_WIDTH, maxByContainer),
    )
  }

  function clampSidebarWidth(width: number) {
    return clampValue(width, SIDEBAR_MIN_WIDTH, getSidebarMaxWidth())
  }

  function setSidebarWidthIfNeeded(nextWidth: number) {
    setSidebarWidth((currentWidth) =>
      currentWidth === nextWidth ? currentWidth : nextWidth,
    )
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
    window.addEventListener("resize", syncSidebarWidthToViewport)
    return () => {
      window.removeEventListener("resize", syncSidebarWidthToViewport)
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
    document.body.style.userSelect = "none"
    document.body.style.cursor = "col-resize"
    return () => {
      document.body.style.userSelect = currentUserSelect
      document.body.style.cursor = currentCursor
    }
  }, [isResizingSidebar])

  if (!initialized) {
    return <LoadingScreen />
  }

  if (providers.length === 0) {
    return <Navigate replace to="/welcome/provider" />
  }

  function updateSettingsQuery(nextOpen: boolean) {
    const nextParams = new URLSearchParams(searchParams)
    if (nextOpen) {
      nextParams.set("settings", "1")
    } else {
      nextParams.delete("settings")
    }
    setSearchParams(nextParams, { replace: !nextOpen })
  }

  async function handleCreateSession() {
    const fallbackProvider = providers[0] ?? null
    const providerForNewSession = activeSession?.provider_profile_id
      ? (providers.find(
          (item) => item.id === activeSession.provider_profile_id,
        ) ?? fallbackProvider)
      : fallbackProvider

    const created = await getAppClient().request<{ id: string }>(
      "sessions.create",
      {
        provider_profile_id: providerForNewSession?.id ?? null,
      },
    )

    navigate({
      pathname: `/chat/${created.id}`,
      search: isSettingsOpen ? "?settings=1" : "",
    })
  }

  async function handleDeleteSession(targetSessionId: string) {
    await getAppClient().request("sessions.delete", {
      session_id: targetSessionId,
    })

    if (targetSessionId === sessionIdFromRoute) {
      navigate({ pathname: "/", search: isSettingsOpen ? "?settings=1" : "" })
    }
  }

  async function handleRenameSession(targetSessionId: string, title: string) {
    await getAppClient().request("sessions.rename", {
      session_id: targetSessionId,
      title,
    })
  }

  function handleSelectSession(targetSessionId: string) {
    navigate({
      pathname: `/chat/${targetSessionId}`,
      search: isSettingsOpen ? "?settings=1" : "",
    })
  }

  function handleSidebarResizePointerDown(
    event: PointerEvent<HTMLButtonElement>,
  ) {
    resizeStartRef.current = {
      startX: event.clientX,
      startWidth: sidebarWidth,
    }
    setIsResizingSidebar(true)
    event.currentTarget.setPointerCapture(event.pointerId)
  }

  function handleSidebarResizePointerMove(
    event: PointerEvent<HTMLButtonElement>,
  ) {
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
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
      return
    }
    event.preventDefault()
    const delta =
      event.key === "ArrowRight" ? SIDEBAR_RESIZE_STEP : -SIDEBAR_RESIZE_STEP
    setSidebarWidth((currentWidth) => clampSidebarWidth(currentWidth + delta))
  }
  const handlePosition = sidebarWidth

  return (
    <main className="box-border h-dvh select-none overflow-hidden bg-layout">
      <div
        className="relative grid h-full min-h-0 overflow-hidden bg-background/65"
        ref={shellRef}
        style={{
          gridTemplateColumns: `${sidebarWidth}px minmax(0, 1fr)`,
        }}
      >
        <div className="min-h-0 overflow-hidden">
          <SessionSidebar
            activeSessionId={selectedSidebarSessionId}
            activeView={isSettingsOpen ? "settings" : "chat"}
            onCreateSession={() => void handleCreateSession()}
            onDeleteSession={(id) => void handleDeleteSession(id)}
            onOpenSettings={() => updateSettingsQuery(true)}
            onRenameSession={handleRenameSession}
            onSelectSession={handleSelectSession}
            providers={providers}
            sessions={sessions}
          />
        </div>

        <section
          className="min-h-0 select-none overflow-hidden rounded-l-xl bg-background/85"
          key={location.pathname}
        >
          <Outlet />
        </section>

        <button
          aria-label="调整侧边栏宽度"
          aria-orientation="vertical"
          className={cn(
            "group absolute top-1/2 z-20 flex h-10 w-4 -translate-x-1/2 -translate-y-1/2 cursor-col-resize items-center justify-center rounded-full bg-background/85 text-muted-foreground shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
            isResizingSidebar
              ? "border-foreground/25 bg-accent/50 text-foreground"
              : "hover:bg-card hover:text-foreground",
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
          type="button"
        >
          <GripVertical className="h-3.5 w-3.5" />
        </button>
      </div>

      <SettingsModal onOpenChange={updateSettingsQuery} open={isSettingsOpen} />
    </main>
  )
}
