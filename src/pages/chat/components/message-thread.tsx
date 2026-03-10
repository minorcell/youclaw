import { code } from "@streamdown/code"
import { Bot, TriangleAlert } from "lucide-react"
import { Streamdown } from "streamdown"

import { Card } from "@/components/ui/card"
import { partsToOutputText, reasoningParts, visibleMessages } from "@/lib/parts"
import type { ChatMessage } from "@/lib/types"

const streamdownPlugins = { code }

interface MessageThreadProps {
  messages: ChatMessage[]
  providerLabel?: string
  runSteps?: Array<{
    step: number
    status: "started" | "finished"
    outputText: string
    reasoningText: string
  }>
  error?: string
}

export function MessageThread({
  messages,
  providerLabel = "Agent",
  runSteps = [],
  error,
}: MessageThreadProps) {
  const items = visibleMessages(messages)

  return (
    <div className="space-y-6">
      {items.map((message) => {
        const isUser = message.role === "user"
        const outputText = partsToOutputText(message.parts_json)
        const reasoningText = reasoningParts(message.parts_json)
          .map((part) => {
            if (part.text) return part.text
            const anthropic = part.provider_metadata?.anthropic as
              | { redacted_data?: unknown }
              | undefined
            if (anthropic?.redacted_data) {
              return "[reasoning redacted by provider]"
            }
            return ""
          })
          .join("")
        const toolSummary = message.parts_json
          .flatMap((part) => ("ToolCall" in part ? [`[tool:${part.ToolCall.tool_name}]`] : []))
          .join("\n")

        if (isUser) {
          return (
            <div className="flex justify-end" key={message.id}>
              <Streamdown
                className="max-w-[68ch] text-sm leading-7 text-foreground"
                controls={false}
                mode="static"
                plugins={streamdownPlugins}
              >
                {outputText}
              </Streamdown>
            </div>
          )
        }

        return (
          <article className="max-w-[76ch]" key={message.id}>
            <div className="mb-2 flex items-center gap-2 text-sm text-muted-foreground">
              <Bot className="h-4 w-4" />
              <span className="font-medium">{providerLabel}</span>
            </div>
            {reasoningText ? (
              <details className="mb-3 rounded-2xl border border-border/70 bg-muted/30 px-4 py-3 text-sm">
                <summary className="cursor-pointer text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                  Model reasoning
                </summary>
                <Streamdown
                  className="mt-3 text-sm leading-7 text-muted-foreground"
                  controls={false}
                  mode="static"
                  plugins={streamdownPlugins}
                >
                  {reasoningText}
                </Streamdown>
              </details>
            ) : null}
            <Streamdown
              className="text-base leading-8 text-foreground"
              controls={false}
              mode="static"
              plugins={streamdownPlugins}
            >
              {outputText || toolSummary}
            </Streamdown>
          </article>
        )
      })}

      {runSteps.map((step) => (
        <article className="max-w-[76ch]" key={`live-step-${step.step}`}>
          <div className="mb-2 flex items-center gap-2 text-sm text-muted-foreground">
            <Bot className="h-4 w-4" />
            <span className="font-medium">{providerLabel}</span>
          </div>
          {step.reasoningText ? (
            <details className="mb-3 rounded-2xl border border-border/70 bg-muted/30 px-4 py-3 text-sm">
              <summary className="cursor-pointer text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                Model reasoning
              </summary>
              <Streamdown
                className="mt-3 text-xs leading-6 text-muted-foreground"
                controls={false}
                isAnimating={step.status === "started"}
                plugins={streamdownPlugins}
              >
                {step.reasoningText}
              </Streamdown>
            </details>
          ) : null}
          <Streamdown
            caret="block"
            className="text-sm leading-7 text-foreground"
            controls={false}
            isAnimating={step.status === "started"}
            plugins={streamdownPlugins}
          >
            {step.outputText}
          </Streamdown>
        </article>
      ))}

      {error ? (
        <Card className="max-w-[76ch] border-destructive/20 bg-destructive/10 px-4 py-3 text-sm text-destructive shadow-none">
          <div className="flex items-start gap-2">
            <TriangleAlert className="mt-0.5 h-4 w-4" />
            <span>{error}</span>
          </div>
        </Card>
      ) : null}
    </div>
  )
}
