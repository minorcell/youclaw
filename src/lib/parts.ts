import type { ChatMessage, ContentPart } from "@/lib/types"

export function partsToText(parts: ContentPart[]): string {
  return parts
    .map((part) => {
      if ("Text" in part) {
        return part.Text
      }
      if ("ToolCall" in part) {
        return `[tool:${part.ToolCall.tool_name}]`
      }
      return part.ToolResult.is_error ? "[tool:error]" : "[tool:ok]"
    })
    .join("\n")
}

export function visibleMessages(messages: ChatMessage[]): ChatMessage[] {
  return messages.filter((message) => message.role !== "system")
}
