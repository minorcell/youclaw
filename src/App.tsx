import { invoke } from "@tauri-apps/api/core"
import { useEffect } from "react"
import { RouterProvider, createHashRouter } from "react-router-dom"

import { AppLayout } from "@/components/layouts"
import { setAppClient } from "@/lib/app-client"
import { applyTheme } from "@/lib/theme"
import { AppWsClient } from "@/lib/ws-client"
import { ChatPage } from "@/pages/chat"
import { HomeRedirectPage, ProviderOnboardingPage } from "@/pages/welcome"
import { SettingsPage } from "@/pages/settings"
import { useAppStore } from "@/store/app-store"
import { useThemeStore } from "@/store/theme-store"

const router = createHashRouter([
  {
    path: "/welcome/provider",
    element: <ProviderOnboardingPage />,
  },
  {
    path: "/",
    element: <AppLayout />,
    children: [
      {
        index: true,
        element: <HomeRedirectPage />,
      },
      {
        path: "chat/:sessionId",
        element: <ChatPage />,
      },
    ],
  },
  {
    path: "/settings",
    element: <SettingsPage />,
  },
])

export default function App() {
  const setEndpoint = useAppStore((state) => state.setEndpoint)
  const setWsStatus = useAppStore((state) => state.setWsStatus)
  const themeMode = useThemeStore((state) => state.mode)
  const customTheme = useThemeStore((state) => state.custom)

  useEffect(() => {
    let disposed = false
    let client: AppWsClient | null = null

    async function bootstrap() {
      const endpoint = await invoke<string>("get_ws_endpoint")
      if (disposed) {
        return
      }
      setEndpoint(endpoint)
      const nextClient = new AppWsClient({
        endpoint,
        onEnvelope: (envelope) => {
          if (disposed) {
            return
          }
          useAppStore.getState().applyEnvelope(envelope)
        },
        onStatusChange: (status) => {
          if (disposed) {
            return
          }
          setWsStatus(status)
          if (status === "open") {
            void nextClient.request("bootstrap.get", {}).catch((error) => {
              if (disposed) {
                return
              }
              useAppStore.getState().applyEnvelope({
                id: crypto.randomUUID(),
                kind: "response",
                name: "bootstrap.get",
                payload: null,
                ok: false,
                error: { code: "bootstrap_failed", message: String(error) },
              })
            })
          }
        },
      })
      if (disposed) {
        nextClient.disconnect()
        return
      }
      client = nextClient
      setAppClient(nextClient)
      nextClient.connect()
    }

    void bootstrap()

    return () => {
      disposed = true
      client?.disconnect()
      setAppClient(null)
    }
  }, [setEndpoint, setWsStatus])

  useEffect(() => {
    applyTheme(themeMode, customTheme)
  }, [themeMode, customTheme])

  return <RouterProvider router={router} />
}
