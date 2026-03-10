import { Navigate } from "react-router-dom"

import { useAppStore } from "@/store/app-store"

export function SettingsPage() {
  const initialized = useAppStore((state) => state.initialized)
  const providers = useAppStore((state) => state.providers)
  const sessions = useAppStore((state) => state.sessions)
  const activeSessionId = useAppStore((state) => state.activeSessionId)
  const lastOpenedSessionId = useAppStore((state) => state.lastOpenedSessionId)

  if (!initialized) {
    return <div className="h-screen overflow-hidden bg-background" />
  }

  if (providers.length === 0) {
    return <Navigate replace to="/welcome/provider" />
  }

  const targetSessionId =
    activeSessionId ?? lastOpenedSessionId ?? sessions[0]?.id ?? null

  if (!targetSessionId) {
    return <Navigate replace to="/?settings=1" />
  }

  return <Navigate replace to={`/chat/${targetSessionId}?settings=1`} />
}
