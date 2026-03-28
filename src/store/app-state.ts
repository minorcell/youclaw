import type {
  ChatMessage,
  ChatSession,
  ProviderAccount,
  ToolApproval,
  WorkspaceRootInfo,
} from '@/lib/types'
import type { TurnViewState } from '@/store/types'

export type WsStatus = 'idle' | 'connecting' | 'open' | 'closed' | 'error'

export interface AppStoreData {
  initialized: boolean
  wsStatus: WsStatus
  endpoint: string | null
  providerAccounts: ProviderAccount[]
  sessions: ChatSession[]
  recentWorkspaces: WorkspaceRootInfo[]
  messagesBySession: Record<string, ChatMessage[]>
  approvalsById: Record<string, ToolApproval>
  turnsById: Record<string, TurnViewState>
  activeTurnIdBySession: Record<string, string>
  activeSessionId: string | null
  lastOpenedSessionId: string | null
  lastError: string | null
}

export function createInitialAppStoreData(): AppStoreData {
  return {
    initialized: false,
    wsStatus: 'idle',
    endpoint: null,
    providerAccounts: [],
    sessions: [],
    recentWorkspaces: [],
    messagesBySession: {},
    approvalsById: {},
    turnsById: {},
    activeTurnIdBySession: {},
    activeSessionId: null,
    lastOpenedSessionId: null,
    lastError: null,
  }
}
