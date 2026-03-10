import { SendHorizonal } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Textarea } from "@/components/ui/textarea"
import type { ProviderProfile } from "@/lib/types"

interface ChatComposerProps {
  input: string
  providers: ProviderProfile[]
  selectedProviderId: string | null
  onInputChange: (value: string) => void
  onSend: () => void
  onBindProvider: (providerProfileId: string | null) => void
}

export function ChatComposer({
  input,
  providers,
  selectedProviderId,
  onInputChange,
  onSend,
  onBindProvider,
}: ChatComposerProps) {
  return (
    <Card className="rounded-3xl border-border/80 bg-background/95 py-0 shadow-[0_12px_40px_-26px_rgba(0,0,0,0.35)] backdrop-blur">
      <div className="px-5 pt-4">
        <Textarea
          className="min-h-[74px] resize-none border-0 bg-transparent p-0 text-[16px] leading-7 shadow-none focus-visible:ring-0"
          onChange={(event) => onInputChange(event.target.value)}
          onKeyDown={(event) => {
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
              event.preventDefault()
              onSend()
            }
          }}
          placeholder="输入消息...（输入 / 召唤牛马）"
          value={input}
        />
      </div>

      <div className="flex items-center justify-end gap-3 px-4 py-3">
        <div className="flex items-center gap-2">
          <Select
            onValueChange={(value) => onBindProvider(value)}
            value={selectedProviderId ?? providers[0]?.id ?? ""}
          >
            <SelectTrigger className="h-9 min-w-[170px] rounded-full border-border bg-muted/70 px-3 text-[13px]">
              <SelectValue placeholder="选择模型" />
            </SelectTrigger>
            <SelectContent>
              {providers.map((provider) => (
                <SelectItem key={provider.id} value={provider.id}>
                  {provider.model}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button
            className="h-10 w-10 rounded-full bg-primary text-primary-foreground hover:bg-primary/90"
            onClick={onSend}
            size="icon"
            type="button"
          >
            <SendHorizonal className="h-4 w-4" />
          </Button>
        </div>
      </div>
    </Card>
  )
}
