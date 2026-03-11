import type { WsEnvelope } from '@/lib/types'

interface PendingRequest {
  resolve: (value: any) => void
  reject: (error: Error) => void
  skipDispatch?: boolean
}

interface AppWsClientOptions {
  endpoint: string
  onEnvelope: (envelope: WsEnvelope) => void
  onStatusChange: (status: 'connecting' | 'open' | 'closed' | 'error') => void
}

export class AppWsClient {
  private socket: WebSocket | null = null
  private readonly pending = new Map<string, PendingRequest>()
  private reconnectAttempt = 0
  private reconnectTimer: number | null = null
  private heartbeatTimer: number | null = null
  private manualClose = false

  constructor(private readonly options: AppWsClientOptions) {}

  connect() {
    this.manualClose = false
    this.options.onStatusChange('connecting')
    this.socket = new WebSocket(this.options.endpoint)

    this.socket.addEventListener('open', () => {
      this.reconnectAttempt = 0
      this.options.onStatusChange('open')
      this.startHeartbeat()
    })

    this.socket.addEventListener('message', (event) => {
      const envelope = JSON.parse(String(event.data)) as WsEnvelope
      const pending = this.pending.get(envelope.id)
      if (envelope.kind === 'response' && pending) {
        this.pending.delete(envelope.id)
        if (envelope.ok === false && envelope.error) {
          pending.reject(new Error(envelope.error.message))
        } else {
          pending.resolve(envelope.payload)
        }
        if (!pending.skipDispatch) {
          this.options.onEnvelope(envelope)
        }
        return
      }
      this.options.onEnvelope(envelope)
    })

    this.socket.addEventListener('close', () => {
      this.stopHeartbeat()
      this.options.onStatusChange('closed')
      if (!this.manualClose) {
        this.scheduleReconnect()
      }
    })

    this.socket.addEventListener('error', () => {
      this.options.onStatusChange('error')
    })
  }

  disconnect() {
    this.manualClose = true
    this.stopHeartbeat()
    if (this.reconnectTimer) {
      window.clearTimeout(this.reconnectTimer)
      this.reconnectTimer = null
    }
    this.socket?.close()
    this.socket = null
  }

  async request<TResponse = unknown>(
    name: string,
    payload: unknown,
    options?: { skipDispatch?: boolean },
  ): Promise<TResponse> {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      throw new Error('WebSocket is not connected')
    }

    const envelope: WsEnvelope = {
      id: crypto.randomUUID(),
      kind: 'request',
      name,
      payload,
    }

    return new Promise<TResponse>((resolve, reject) => {
      this.pending.set(envelope.id, { resolve, reject, skipDispatch: options?.skipDispatch })
      this.socket?.send(JSON.stringify(envelope))
    })
  }

  private startHeartbeat() {
    this.stopHeartbeat()
    this.heartbeatTimer = window.setInterval(() => {
      void this.request('bootstrap.get', { heartbeat: true }, { skipDispatch: true }).catch(() => {
        this.socket?.close()
      })
    }, 15_000)
  }

  private stopHeartbeat() {
    if (this.heartbeatTimer) {
      window.clearInterval(this.heartbeatTimer)
      this.heartbeatTimer = null
    }
  }

  private scheduleReconnect() {
    if (this.reconnectTimer) {
      return
    }
    const delay = Math.min(1_000 * 2 ** this.reconnectAttempt, 8_000)
    this.reconnectAttempt += 1
    this.reconnectTimer = window.setTimeout(() => {
      this.reconnectTimer = null
      this.connect()
    }, delay)
  }
}
