import { create } from 'zustand'

import type { WsEnvelope } from '@/lib/types'
import { createInitialAppStoreData, type AppStoreData, type WsStatus } from '@/store/app-state'
import { reduceEnvelopeData } from '@/store/reducers/envelope-reducer'

export type { WsStatus }

interface AppStoreState extends AppStoreData {
  setWsStatus: (status: WsStatus) => void
  setEndpoint: (endpoint: string) => void
  setActiveSession: (sessionId: string | null) => void
  applyEnvelope: (envelope: WsEnvelope) => void
  clearError: () => void
}
export const useAppStore = create<AppStoreState>((set) => ({
  ...createInitialAppStoreData(),
  setWsStatus: (wsStatus) => set({ wsStatus }),
  setEndpoint: (endpoint) => set({ endpoint }),
  setActiveSession: (activeSessionId) => set({ activeSessionId }),
  clearError: () => set({ lastError: null }),
  applyEnvelope: (envelope) => {
    set((state) => {
      const next = reduceEnvelopeData(state, envelope)
      return next ?? state
    })
  },
}))
