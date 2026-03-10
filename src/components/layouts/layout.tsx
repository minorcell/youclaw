import { useMemo } from "react"
import {
  Navigate,
  Outlet,
  useLocation,
  useNavigate,
  useParams,
  useSearchParams,
} from "react-router-dom"

import { SessionSidebar } from "@/components/layouts/sidebar"
import { SettingsModal } from "@/pages/settings/components"
import { getAppClient } from "@/lib/app-client"
import type { ProviderProfile } from "@/lib/types"
import { LoadingScreen } from "@/pages/welcome"
import { useAppStore } from "@/store/app-store"

export function AppLayout() {
  const navigate = useNavigate()
  const location = useLocation()
  const params = useParams<{ sessionId?: string }>()
  const [searchParams, setSearchParams] = useSearchParams()

  const sessionIdFromRoute = params.sessionId ?? null
  const isSettingsOpen = searchParams.get("settings") === "1"

  const initialized = useAppStore((state) => state.initialized)
  const providers = useAppStore((state) => state.providers)
  const sessions = useAppStore((state) => state.sessions)
  const activeSessionId = useAppStore((state) => state.activeSessionId)

  const activeSession = useMemo(
    () => sessions.find((session) => session.id === sessionIdFromRoute) ?? null,
    [sessions, sessionIdFromRoute],
  )

  const selectedSidebarSessionId =
    sessionIdFromRoute ?? activeSessionId ?? sessions[0]?.id ?? null

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
    const providerForNewSession: ProviderProfile | null =
      activeSession?.provider_profile_id
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

  function handleSelectSession(targetSessionId: string) {
    navigate({
      pathname: `/chat/${targetSessionId}`,
      search: isSettingsOpen ? "?settings=1" : "",
    })
  }

  return (
    <main className="box-border h-[100dvh] overflow-hidden bg-[#ecece8]">
      <div className="grid h-full min-h-0 overflow-hidden rounded-xl border border-border/70 bg-background/65 lg:grid-cols-[330px_minmax(0,1fr)]">
        <SessionSidebar
          activeSessionId={selectedSidebarSessionId}
          activeView={isSettingsOpen ? "settings" : "chat"}
          onCreateSession={() => void handleCreateSession()}
          onDeleteSession={(id) => void handleDeleteSession(id)}
          onOpenSettings={() => updateSettingsQuery(true)}
          onSelectSession={handleSelectSession}
          providers={providers}
          sessions={sessions}
        />

        <section
          className="min-h-0 overflow-hidden border-l border-border/70 bg-background/85"
          key={location.pathname}
        >
          <Outlet />
        </section>
      </div>

      <SettingsModal onOpenChange={updateSettingsQuery} open={isSettingsOpen} />
    </main>
  )
}
