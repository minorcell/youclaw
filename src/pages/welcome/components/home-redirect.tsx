import { useEffect, useMemo } from "react"
import { Navigate, useLocation, useNavigate } from "react-router-dom"

import { useToastContext } from "@/contexts/toast-context"
import { getAppClient } from "@/lib/app-client"
import { flattenProviderProfiles } from "@/lib/provider-profiles"
import { useAppStore } from "@/store/app-store"

function errorMessageFromUnknown(error: unknown): string {
  if (typeof error === "string") {
    return error
  }
  if (error instanceof Error) {
    return error.message
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message
  }
  return String(error)
}

export function HomeRedirectPage() {
  const location = useLocation()
  const initialized = useAppStore((state) => state.initialized)
  const providerAccounts = useAppStore((state) => state.providerAccounts)
  const sessions = useAppStore((state) => state.sessions)
  const activeSessionId = useAppStore((state) => state.activeSessionId)
  const lastOpenedSessionId = useAppStore((state) => state.lastOpenedSessionId)
  const providers = useMemo(
    () => flattenProviderProfiles(providerAccounts),
    [providerAccounts],
  )

  if (!initialized) {
    return <LoadingScreen />
  }

  if (providers.length === 0) {
    return <Navigate replace to="/welcome/provider" />
  }

  const targetSessionId =
    activeSessionId ?? lastOpenedSessionId ?? sessions[0]?.id ?? null

  if (targetSessionId) {
    return (
      <Navigate
        replace
        to={{
          pathname: `/chat/${targetSessionId}`,
          search: location.search,
        }}
      />
    )
  }

  return (
    <CreateSessionAndRedirect
      providerProfileId={providers[0]?.id ?? null}
      search={location.search}
    />
  )
}

function CreateSessionAndRedirect({
  providerProfileId,
  search,
}: {
  providerProfileId: string | null
  search: string
}) {
  const navigate = useNavigate()
  const { error: toastError } = useToastContext()

  useEffect(() => {
    let cancelled = false

    async function createSession() {
      try {
        const created = await getAppClient().request<{ id: string }>(
          "sessions.create",
          {
            provider_profile_id: providerProfileId,
          },
        )

        if (cancelled) return

        navigate(
          {
            pathname: `/chat/${created.id}`,
            search,
          },
          { replace: true },
        )
      } catch (nextError) {
        if (cancelled) return
        toastError(errorMessageFromUnknown(nextError))
        navigate("/welcome/provider", { replace: true })
      }
    }

    void createSession()

    return () => {
      cancelled = true
    }
  }, [navigate, providerProfileId, search, toastError])

  return <LoadingScreen />
}

export function LoadingScreen() {
  const wsStatus = useAppStore((state) => state.wsStatus)
  const lastError = useAppStore((state) => state.lastError)

  return (
    <div className="flex min-h-screen items-center justify-center bg-background px-4">
      <div className="max-w-180 rounded-3xl bg-card px-8 py-6 text-center shadow-none">
        <p className="text-xs font-serif uppercase tracking-[0.24em] text-muted-foreground">
          YouClaw
        </p>
        <h1 className="mt-3 text-2xl font-serif font-semibold text-foreground">
          Connecting to local agent runtime…
        </h1>
        <p className="mt-2 text-xs uppercase tracking-[0.18em] text-muted-foreground">
          ws: {wsStatus}
        </p>
        {lastError ? (
          <div className="mt-4 rounded-2xl bg-destructive/10 px-4 py-3 text-left">
            <p className="text-xs uppercase tracking-[0.18em] text-destructive/80">
              bootstrap failed
            </p>
            <p className="mt-1 wrap-break-word text-sm text-destructive">
              {lastError}
            </p>
          </div>
        ) : null}
      </div>
    </div>
  )
}
