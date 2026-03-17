import type { AppStoreData } from '@/store/app-state'
import type { WsEnvelope } from '@/lib/types'

import { reduceBootstrapEnvelope } from './bootstrap-reducer'
import { reduceCatalogEnvelope } from './catalog-events-reducer'
import { reduceChatEnvelope } from './chat-events-reducer'

export function reduceEnvelopeData(
  state: AppStoreData,
  envelope: WsEnvelope,
): Partial<AppStoreData> | null {
  if (envelope.kind === 'response' && envelope.ok === false && envelope.error) {
    return { lastError: envelope.error.message }
  }

  return (
    reduceBootstrapEnvelope(state, envelope) ??
    mergeCatalogPatch(state, reduceCatalogEnvelope(envelope)) ??
    reduceChatEnvelope(state, envelope)
  )
}

function mergeCatalogPatch(
  state: AppStoreData,
  patch: Partial<AppStoreData> | null,
): Partial<AppStoreData> | null {
  if (!patch) {
    return null
  }

  if (patch.approvalsById) {
    return {
      ...patch,
      approvalsById: {
        ...state.approvalsById,
        ...patch.approvalsById,
      },
    }
  }

  return patch
}
