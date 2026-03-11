import type { ChatMessage, ContentPart, ReasoningPart } from '@/lib/types'

export function partsToText(parts: ContentPart[]): string {
  return parts
    .map((part) => {
      if ('Text' in part) {
        return part.Text
      }
      if ('Reasoning' in part) {
        return part.Reasoning.text
      }
      if ('ToolCall' in part) {
        return `[tool:${part.ToolCall.tool_name}]`
      }
      return part.ToolResult.is_error ? '[tool:error]' : '[tool:ok]'
    })
    .join('\n')
}

export function partsToOutputText(parts: ContentPart[]): string {
  return parts.flatMap((part) => ('Text' in part ? [part.Text] : [])).join('')
}

export function partsToReasoningText(parts: ContentPart[]): string {
  return parts.flatMap((part) => ('Reasoning' in part ? [part.Reasoning.text] : [])).join('')
}

export function reasoningParts(parts: ContentPart[]): ReasoningPart[] {
  return parts.flatMap((part) => ('Reasoning' in part ? [part.Reasoning] : []))
}

export function partsToReasoningDisplay(parts: ContentPart[]): string {
  return reasoningParts(parts)
    .map((part) => {
      if (part.text) return part.text
      const anthropic = part.provider_metadata?.anthropic as { redacted_data?: unknown } | undefined
      if (anthropic?.redacted_data) return '[reasoning redacted by provider]'
      return ''
    })
    .join('')
}

export function visibleMessages(messages: ChatMessage[]): ChatMessage[] {
  return messages.filter((message) => message.role !== 'system')
}
