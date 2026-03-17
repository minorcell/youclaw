import { useEffect, useMemo, useRef } from 'react'

import type { TurnRenderUnit } from '../types'

export function useChatScroll(turnRenderUnits: TurnRenderUnit[]) {
  const scrollContainerRef = useRef<HTMLDivElement>(null)
  const scrolledUpRef = useRef(false)
  const lastTurnCountRef = useRef(0)
  const lastStepTextRef = useRef('')

  useEffect(() => {
    const container = scrollContainerRef.current
    if (!container) {
      return
    }

    const handleWheel = (event: WheelEvent) => {
      if (event.deltaY < 0) {
        scrolledUpRef.current = true
      }
    }

    const handleScroll = () => {
      const { scrollTop, scrollHeight, clientHeight } = container
      if (scrollHeight - scrollTop - clientHeight < 60) {
        scrolledUpRef.current = false
      }
    }

    container.addEventListener('wheel', handleWheel, { passive: true })
    container.addEventListener('scroll', handleScroll, { passive: true })

    return () => {
      container.removeEventListener('wheel', handleWheel)
      container.removeEventListener('scroll', handleScroll)
    }
  }, [])

  const turnCount = turnRenderUnits.length
  useEffect(() => {
    const container = scrollContainerRef.current
    if (!container || scrolledUpRef.current) {
      return
    }
    if (turnCount > lastTurnCountRef.current) {
      container.scrollTo({ top: container.scrollHeight })
    }
    lastTurnCountRef.current = turnCount
  }, [turnCount])

  const lastStepText = useMemo(() => {
    const lastTurn = turnRenderUnits[turnRenderUnits.length - 1]
    if (!lastTurn) {
      return ''
    }
    const lastStep = lastTurn.steps[lastTurn.steps.length - 1]
    if (!lastStep) {
      return ''
    }
    return `${lastStep.reasoningText}${lastStep.outputText}`
  }, [turnRenderUnits])

  useEffect(() => {
    if (lastStepText === lastStepTextRef.current) {
      return
    }
    lastStepTextRef.current = lastStepText

    const container = scrollContainerRef.current
    if (!container || scrolledUpRef.current) {
      return
    }

    requestAnimationFrame(() => {
      if (!scrolledUpRef.current) {
        container.scrollTo({ top: container.scrollHeight })
      }
    })
  }, [lastStepText])

  return {
    scrollContainerRef,
    resetAutoScroll() {
      scrolledUpRef.current = false
    },
  }
}
