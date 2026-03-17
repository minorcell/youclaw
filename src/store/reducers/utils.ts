import type { BootstrapPayload, ChatMessage, ChatTurn } from '@/lib/types'
import type { AppStoreData } from '@/store/app-state'
import type { TimelineItem, TurnViewState } from '@/store/types'

export function groupMessages(messages: ChatMessage[]): Record<string, ChatMessage[]> {
  const seenBySession: Record<string, Set<string>> = {}
  return messages.reduce<Record<string, ChatMessage[]>>((acc, message) => {
    acc[message.session_id] ??= []
    seenBySession[message.session_id] ??= new Set<string>()
    if (seenBySession[message.session_id].has(message.id)) {
      return acc
    }
    seenBySession[message.session_id].add(message.id)
    acc[message.session_id].push(message)
    return acc
  }, {})
}

export function mergeUniqueMessages(
  messages: ChatMessage[],
  incoming: ChatMessage[],
): ChatMessage[] {
  if (incoming.length === 0) {
    return messages
  }
  const seen = new Set(messages.map((item) => item.id))
  const next = [...messages]
  let changed = false
  for (const message of incoming) {
    if (seen.has(message.id)) {
      continue
    }
    seen.add(message.id)
    next.push(message)
    changed = true
  }
  return changed ? next : messages
}

export function upsertTimelineItem(items: TimelineItem[], nextItem: TimelineItem): TimelineItem[] {
  const index = items.findIndex((item) => item.id === nextItem.id)
  if (index === -1) {
    return [...items, nextItem]
  }

  const clone = [...items]
  clone[index] = nextItem
  return clone
}

export function getOrCreateTurnView(
  state: Pick<AppStoreData, 'turnsById'>,
  turn: ChatTurn,
): TurnViewState {
  return (
    state.turnsById[turn.id] ?? {
      turn,
      sessionId: turn.session_id,
      timeline: [],
      liveStepsById: {},
      usageTotal: undefined,
      error: turn.error_message ?? undefined,
    }
  )
}

export function buildActiveTurnIdBySession(turns: ChatTurn[]): Record<string, string> {
  const activeTurnBySession: Record<string, ChatTurn> = {}
  for (const turn of turns) {
    const current = activeTurnBySession[turn.session_id]
    if (!current || turn.created_at.localeCompare(current.created_at) > 0) {
      activeTurnBySession[turn.session_id] = turn
    }
  }
  return Object.fromEntries(
    Object.entries(activeTurnBySession).map(([sessionId, turn]) => [sessionId, turn.id]),
  )
}

export function buildTurnMapFromBootstrap(
  payload: BootstrapPayload,
): Record<string, TurnViewState> {
  return Object.fromEntries(
    payload.turns.map((turn) => [
      turn.id,
      {
        turn,
        sessionId: turn.session_id,
        timeline: [],
        liveStepsById: {},
        error: turn.error_message ?? undefined,
      },
    ]),
  )
}
