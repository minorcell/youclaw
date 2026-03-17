import type { BootstrapPayload, WsEnvelope } from '@/lib/types'
import type { AppStoreData } from '@/store/app-state'

import { buildActiveTurnIdBySession, buildTurnMapFromBootstrap, groupMessages } from './utils'

export function reduceBootstrapEnvelope(
  state: AppStoreData,
  envelope: WsEnvelope,
): Partial<AppStoreData> | null {
  if (envelope.name !== 'bootstrap.get') {
    return null
  }

  const payload = envelope.payload as BootstrapPayload
  if (!payload || !('provider_accounts' in payload)) {
    return null
  }

  return {
    initialized: true,
    providerAccounts: payload.provider_accounts ?? [],
    sessions: payload.sessions,
    messagesBySession: groupMessages(payload.messages),
    approvalsById: Object.fromEntries(payload.approvals.map((approval) => [approval.id, approval])),
    turnsById: buildTurnMapFromBootstrap(payload),
    activeTurnIdBySession: buildActiveTurnIdBySession(payload.turns),
    lastOpenedSessionId: payload.last_opened_session_id,
    activeSessionId:
      state.activeSessionId ?? payload.last_opened_session_id ?? payload.sessions[0]?.id ?? null,
    lastError: null,
  }
}
