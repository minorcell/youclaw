import { memo, useMemo } from 'react'
import { code } from '@streamdown/code'
import { Bot, Hammer, TriangleAlert } from 'lucide-react'
import { Streamdown } from 'streamdown'

import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'
import { partsToOutputText, reasoningParts, visibleMessages } from '@/lib/parts'
import type { ChatMessage, ToolCall, ToolResult } from '@/lib/types'

const streamdownPlugins = { code }

function extractToolCalls(message: ChatMessage): ToolCall[] {
  return message.parts_json.flatMap((part) => ('ToolCall' in part ? [part.ToolCall] : []))
}

function extractToolResults(message: ChatMessage): ToolResult[] {
  return message.parts_json.flatMap((part) => ('ToolResult' in part ? [part.ToolResult] : []))
}

interface MessageThreadProps {
  messages: ChatMessage[]
  providerLabel?: string
  runSteps?: Array<{
    step: number
    status: 'started' | 'finished'
    outputText: string
    reasoningText: string
  }>
  error?: string
}

interface MergedMessageItem {
  message: ChatMessage
  toolCalls: ToolCall[]
  toolResults: ToolResult[]
}

interface PreparedMessageItem extends MergedMessageItem {
  isToolMessage: boolean
  isUser: boolean
  outputText: string
  reasoningText: string
  toolResultByCallId: Map<string, ToolResult>
  unmatchedToolResults: ToolResult[]
  hasRenderableContent: boolean
}

function MessageThreadView({
  messages,
  providerLabel = 'Agent',
  runSteps = [],
  error,
}: MessageThreadProps) {
  const mergedItems = useMemo<MergedMessageItem[]>(() => {
    const items = visibleMessages(messages)
    const consumedToolMessageIndexes = new Set<number>()
    const merged: MergedMessageItem[] = []

    for (let index = 0; index < items.length; index += 1) {
      if (consumedToolMessageIndexes.has(index)) {
        continue
      }

      const message = items[index]
      const toolCalls = extractToolCalls(message)
      let toolResults = extractToolResults(message)

      // Pair assistant tool calls with following tool-result messages from same run.
      if (message.role === 'assistant' && toolCalls.length > 0) {
        const toolResultByCallId = new Map<string, ToolResult>(
          toolResults.map((result) => [result.call_id, result]),
        )
        let matchedCount = toolCalls.filter((call) => toolResultByCallId.has(call.call_id)).length

        for (let next = index + 1; next < items.length; next += 1) {
          const nextMessage = items[next]
          if (nextMessage.role !== 'tool' || nextMessage.run_id !== message.run_id) {
            break
          }

          consumedToolMessageIndexes.add(next)
          const nextResults = extractToolResults(nextMessage)
          for (const result of nextResults) {
            if (toolResultByCallId.has(result.call_id)) {
              continue
            }
            toolResultByCallId.set(result.call_id, result)
            if (toolCalls.some((call) => call.call_id === result.call_id)) {
              matchedCount += 1
            }
          }

          if (matchedCount >= toolCalls.length) {
            break
          }
        }

        toolResults = Array.from(toolResultByCallId.values())
      }

      merged.push({
        message,
        toolCalls,
        toolResults,
      })
    }

    return merged
  }, [messages])

  const preparedItems = useMemo<PreparedMessageItem[]>(() => {
    return mergedItems.map(({ message, toolCalls, toolResults }) => {
      const outputText = partsToOutputText(message.parts_json)
      const reasoningText = reasoningParts(message.parts_json)
        .map((part) => {
          if (part.text) return part.text
          const anthropic = part.provider_metadata?.anthropic as
            | { redacted_data?: unknown }
            | undefined
          if (anthropic?.redacted_data) {
            return '[reasoning redacted by provider]'
          }
          return ''
        })
        .join('')
      const toolResultByCallId = new Map<string, ToolResult>(
        toolResults.map((result) => [result.call_id, result]),
      )
      const toolCallIds = new Set(toolCalls.map((call) => call.call_id))
      const unmatchedToolResults = toolResults.filter((result) => !toolCallIds.has(result.call_id))

      return {
        message,
        toolCalls,
        toolResults,
        isToolMessage: message.role === 'tool',
        isUser: message.role === 'user',
        outputText,
        reasoningText,
        toolResultByCallId,
        unmatchedToolResults,
        hasRenderableContent:
          outputText.length > 0 ||
          reasoningText.length > 0 ||
          toolCalls.length > 0 ||
          toolResults.length > 0,
      }
    })
  }, [mergedItems])

  return (
    <div className='space-y-6'>
      {preparedItems.map(
        ({
          message,
          toolCalls,
          isToolMessage,
          isUser,
          outputText,
          reasoningText,
          toolResultByCallId,
          unmatchedToolResults,
          hasRenderableContent,
        }) => {
          if (isUser) {
            return (
              <div className='flex justify-end' key={message.id}>
                <Streamdown
                  className='max-w-[68ch] text-sm leading-7 text-foreground'
                  controls={false}
                  mode='static'
                  plugins={streamdownPlugins}
                >
                  {outputText}
                </Streamdown>
              </div>
            )
          }

          if (!hasRenderableContent) {
            return null
          }

          return (
            <article className='max-w-[76ch]' key={message.id}>
              <div className='mb-2 flex items-center gap-2 text-sm text-muted-foreground'>
                {isToolMessage ? (
                  <>
                    <Hammer className='h-4 w-4' />
                    <span className='font-medium'>Tool</span>
                  </>
                ) : (
                  <>
                    <Bot className='h-4 w-4' />
                    <span className='font-medium'>{providerLabel}</span>
                  </>
                )}
              </div>
              {reasoningText ? (
                <details className='mb-3 rounded-2xl border border-border/70 bg-muted/30 px-4 py-3 text-sm'>
                  <summary className='cursor-pointer text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground'>
                    Model reasoning
                  </summary>
                  <Streamdown
                    className='mt-3 text-sm leading-7 text-muted-foreground'
                    controls={false}
                    mode='static'
                    plugins={streamdownPlugins}
                  >
                    {reasoningText}
                  </Streamdown>
                </details>
              ) : null}
              {outputText ? (
                <Streamdown
                  className='text-base leading-8 text-foreground'
                  controls={false}
                  mode='static'
                  plugins={streamdownPlugins}
                >
                  {outputText}
                </Streamdown>
              ) : null}
              {toolCalls.length > 0 ? (
                <div className='mt-3 space-y-2'>
                  {toolCalls.map((call) => {
                    const matchedResult = toolResultByCallId.get(call.call_id)
                    return (
                      <details
                        className='overflow-hidden rounded-2xl border border-border/70 bg-muted/30'
                        key={`${message.id}-tool-call-${call.call_id}`}
                      >
                        <summary className='flex cursor-pointer list-none items-center justify-between gap-2 px-3 py-2 text-xs [&::-webkit-details-marker]:hidden'>
                          <div className='flex items-center gap-2'>
                            <Badge variant='secondary'>Tool Call</Badge>
                            <Badge>{call.tool_name}</Badge>
                            {matchedResult ? (
                              <Badge variant={matchedResult.is_error ? 'destructive' : 'default'}>
                                {matchedResult.is_error ? 'Error' : 'OK'}
                              </Badge>
                            ) : null}
                          </div>
                          <span className='text-muted-foreground'>点击展开</span>
                        </summary>
                        <pre className='no-scrollbar max-h-72 overflow-auto p-3 text-[11px] leading-5 text-foreground/90'>
                          {JSON.stringify(call.args_json, null, 2)}
                        </pre>
                        {matchedResult ? (
                          <div className='border-t border-border/70'>
                            <div className='flex items-center gap-2 px-3 pt-2 text-xs'>
                              <Badge variant='secondary'>Tool Result</Badge>
                            </div>
                            <pre className='no-scrollbar max-h-72 overflow-auto p-3 text-[11px] leading-5 text-foreground/90'>
                              {JSON.stringify(matchedResult.output_json, null, 2)}
                            </pre>
                          </div>
                        ) : null}
                      </details>
                    )
                  })}
                </div>
              ) : null}
              {unmatchedToolResults.length > 0 ? (
                <div className='mt-3 space-y-2'>
                  {unmatchedToolResults.map((result) => (
                    <details
                      className='overflow-hidden rounded-2xl border border-border/70 bg-muted/30'
                      key={`${message.id}-tool-result-${result.call_id}`}
                    >
                      <summary className='flex cursor-pointer list-none items-center justify-between gap-2 px-3 py-2 text-xs [&::-webkit-details-marker]:hidden'>
                        <div className='flex items-center gap-2'>
                          <Badge variant='secondary'>Tool Result</Badge>
                          <Badge variant={result.is_error ? 'destructive' : 'default'}>
                            {result.is_error ? 'Error' : 'OK'}
                          </Badge>
                        </div>
                        <span className='text-muted-foreground'>点击展开</span>
                      </summary>
                      <div className='border-t border-border/70'>
                        <div className='flex items-center gap-2 px-3 pt-2 text-xs'>
                          <Badge variant='secondary'>Tool Result</Badge>
                          <Badge variant={result.is_error ? 'destructive' : 'default'}>
                            {result.is_error ? 'Error' : 'OK'}
                          </Badge>
                        </div>
                        <pre className='no-scrollbar max-h-72 overflow-auto p-3 text-[11px] leading-5 text-foreground/90'>
                          {JSON.stringify(result.output_json, null, 2)}
                        </pre>
                      </div>
                    </details>
                  ))}
                </div>
              ) : null}
            </article>
          )
        },
      )}

      {runSteps.map((step) => (
        <article className='max-w-[76ch]' key={`live-step-${step.step}`}>
          <div className='mb-2 flex items-center gap-2 text-sm text-muted-foreground'>
            <Bot className='h-4 w-4' />
            <span className='font-medium'>{providerLabel}</span>
          </div>
          {step.reasoningText ? (
            <details className='mb-3 rounded-2xl border border-border/70 bg-muted/30 px-4 py-3 text-sm'>
              <summary className='cursor-pointer text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground'>
                Model reasoning
              </summary>
              <Streamdown
                className='mt-3 text-xs leading-6 text-muted-foreground'
                controls={false}
                isAnimating={step.status === 'started'}
                plugins={streamdownPlugins}
              >
                {step.reasoningText}
              </Streamdown>
            </details>
          ) : null}
          <Streamdown
            caret='block'
            className='text-sm leading-7 text-foreground'
            controls={false}
            isAnimating={step.status === 'started'}
            plugins={streamdownPlugins}
          >
            {step.outputText}
          </Streamdown>
        </article>
      ))}

      {error ? (
        <Card className='max-w-[76ch] border-destructive/20 bg-destructive/10 px-4 py-3 text-sm text-destructive shadow-none'>
          <div className='flex items-start gap-2'>
            <TriangleAlert className='mt-0.5 h-4 w-4' />
            <span>{error}</span>
          </div>
        </Card>
      ) : null}
    </div>
  )
}

export const MessageThread = memo(MessageThreadView)
MessageThread.displayName = 'MessageThread'
