import { useEffect, useState } from 'react'

import { getAppClient } from '@/lib/app-client'
import type { AgentStep, TurnStepsListPayload } from '@/lib/types'

export function usePersistedTurnSteps(activeTurnId: string | null) {
  const [persistedStepsByTurnId, setPersistedStepsByTurnId] = useState<Record<string, AgentStep[]>>({})

  useEffect(() => {
    if (!activeTurnId) {
      return
    }

    let disposed = false
    ;(async () => {
      try {
        const payload = await getAppClient().request<TurnStepsListPayload>('chat.turn.steps.list', {
          turn_id: activeTurnId,
        })
        if (disposed) {
          return
        }
        setPersistedStepsByTurnId((current) => ({
          ...current,
          [activeTurnId]: payload.steps.sort((left, right) => left.step - right.step),
        }))
      } catch {
        if (disposed) {
          return
        }
        setPersistedStepsByTurnId((current) => ({
          ...current,
          [activeTurnId]: current[activeTurnId] ?? [],
        }))
      }
    })()

    return () => {
      disposed = true
    }
  }, [activeTurnId])

  return persistedStepsByTurnId
}
