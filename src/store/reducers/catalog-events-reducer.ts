import type {
  ProvidersChangedPayload,
  SessionsChangedPayload,
  ToolApproval,
  WsEnvelope,
} from '@/lib/types'
import type { AppStoreData } from '@/store/app-state'

export function reduceCatalogEnvelope(envelope: WsEnvelope): Partial<AppStoreData> | null {
  switch (envelope.name) {
    case 'providers.changed':
    case 'providers.list': {
      const payload = envelope.payload as ProvidersChangedPayload
      return {
        providerAccounts: payload.provider_accounts ?? [],
      }
    }
    case 'sessions.changed':
    case 'sessions.list': {
      const payload = envelope.payload as SessionsChangedPayload
      return {
        sessions: payload.sessions,
        lastOpenedSessionId: payload.last_opened_session_id,
        recentWorkspaces: payload.recent_workspaces ?? [],
      }
    }
    case 'tool_approvals.resolve': {
      const approval = envelope.payload as ToolApproval
      return {
        approvalsById: {
          [approval.id]: approval,
        },
        lastError: null,
      }
    }
    default:
      return null
  }
}
