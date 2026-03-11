import { invoke } from '@tauri-apps/api/core'
import { useEffect } from 'react'
import { RouterProvider, createHashRouter } from 'react-router-dom'

import { AppLayout } from '@/components/layouts'
import { ToastProvider } from '@/contexts/toast-context'
import { setAppClient } from '@/lib/app-client'
import { applyTheme } from '@/lib/theme'
import { AppWsClient } from '@/lib/ws-client'
import { ChatPage } from '@/pages/chat'
import { HomeRedirectPage, ProviderOnboardingPage } from '@/pages/welcome'
import { SettingsPage } from '@/pages/settings'
import { useAppStore } from '@/store/app-store'
import { useSettingsStore } from '@/store/settings-store'

const router = createHashRouter([
  {
    path: '/welcome/provider',
    element: <ProviderOnboardingPage />,
  },
  {
    path: '/',
    element: <AppLayout />,
    children: [
      {
        index: true,
        element: <HomeRedirectPage />,
      },
      {
        path: 'chat/:sessionId',
        element: <ChatPage />,
      },
    ],
  },
  {
    path: '/settings',
    element: <SettingsPage />,
  },
])

export default function App() {
  const setEndpoint = useAppStore((state) => state.setEndpoint)
  const setWsStatus = useAppStore((state) => state.setWsStatus)
  const themeMode = useSettingsStore((state) => state.mode)
  const customTheme = useSettingsStore((state) => state.custom)
  const themeFontSize = useSettingsStore((state) => state.fontSize)
  const useSerifFont = useSettingsStore((state) => state.useSerif)

  useEffect(() => {
    let disposed = false
    let client: AppWsClient | null = null

    function reportBootstrapError(error: unknown) {
      const message = String(error)
      setWsStatus('error')
      useAppStore.getState().applyEnvelope({
        id: crypto.randomUUID(),
        kind: 'response',
        name: 'bootstrap.get',
        payload: null,
        ok: false,
        error: { code: 'bootstrap_failed', message },
      })
    }

    async function resolveWsEndpointWithRetry() {
      let lastError: unknown = null
      for (let attempt = 0; attempt < 8; attempt += 1) {
        try {
          return await invoke<string>('get_ws_endpoint')
        } catch (error) {
          lastError = error
          await new Promise((resolve) => window.setTimeout(resolve, 150 * (attempt + 1)))
        }
      }
      throw lastError ?? new Error('Unable to resolve ws endpoint')
    }

    async function bootstrap() {
      try {
        const endpoint = await resolveWsEndpointWithRetry()
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
            if (status === 'open') {
              void nextClient.request('bootstrap.get', {}).catch((error) => {
                if (disposed) {
                  return
                }
                reportBootstrapError(error)
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
      } catch (error) {
        if (disposed) {
          return
        }
        reportBootstrapError(error)
      }
    }

    void bootstrap()

    return () => {
      disposed = true
      client?.disconnect()
      setAppClient(null)
    }
  }, [setEndpoint, setWsStatus])

  useEffect(() => {
    applyTheme(themeMode, customTheme, themeFontSize, useSerifFont)
  }, [themeMode, customTheme, themeFontSize, useSerifFont])

  return (
    <ToastProvider>
      <RouterProvider router={router} />
    </ToastProvider>
  )
}
