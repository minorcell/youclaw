import { code } from "@streamdown/code"
import { Bot, TriangleAlert } from "lucide-react"
import { Streamdown } from "streamdown"

import { Card } from "@/components/ui/card"
import { partsToText, visibleMessages } from "@/lib/parts"
import type { ChatMessage } from "@/lib/types"

const streamdownPlugins = { code }

interface MessageThreadProps {
  messages: ChatMessage[]
  providerLabel?: string
  runSteps?: Array<{
    step: number
    status: "started" | "finished"
    outputText: string
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

        if (isUser) {
          return (
            <div className="flex justify-end" key={message.id}>
              <Streamdown
                className="max-w-[68ch] text-sm leading-7 text-foreground"
                controls={false}
                mode="static"
                plugins={streamdownPlugins}
              >
                {partsToText(message.parts_json)}
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
            <Streamdown
              className="text-base leading-8 text-foreground"
              controls={false}
              mode="static"
              plugins={streamdownPlugins}
            >
              {partsToText(message.parts_json)}
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
