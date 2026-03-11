import { AppWsClient } from '@/lib/ws-client'

let appClient: AppWsClient | null = null

export function setAppClient(client: AppWsClient | null) {
  appClient = client
}

export function getAppClient() {
  if (!appClient) {
    throw new Error('App WebSocket client is not initialized')
  }
  return appClient
}
