import { ShieldAlert } from 'lucide-react'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import type { ToolApproval } from '@/lib/types'

interface ToolApprovalCardProps {
  approval: ToolApproval
  onResolveApproval?: (approvalId: string, approved: boolean) => void
}

export function ToolApprovalCard({ approval, onResolveApproval }: ToolApprovalCardProps) {
  const isCommandApproval = approval.preview_json.kind === 'command'
  const title = isCommandApproval ? 'Shell 执行待确认' : '文件写入待确认'

  return (
    <Card className='max-w-[76ch] rounded-2xl border-border/70 bg-card/80 px-4 py-3 shadow-none'>
      <div className='flex items-center justify-between gap-2'>
        <div className='min-w-0'>
          <div className='flex items-center gap-2 text-sm font-medium text-foreground'>
            <ShieldAlert className='h-4 w-4 text-muted-foreground' />
            <span>{title}</span>
          </div>
          <p className='mt-1 truncate text-sm text-foreground'>{approval.subject}</p>
        </div>
        <Badge>{approval.action}</Badge>
      </div>

      {isCommandApproval ? (
        <div className='mt-3 space-y-2'>
          <div className='flex flex-wrap items-center gap-2 text-xs text-muted-foreground'>
            {approval.preview_json.cwd ? <span>cwd: {approval.preview_json.cwd}</span> : null}
            {typeof approval.preview_json.timeout_ms === 'number' ? (
              <span>timeout: {approval.preview_json.timeout_ms} ms</span>
            ) : null}
          </div>
          {approval.preview_json.risk_flags?.length ? (
            <div className='flex flex-wrap gap-2'>
              {approval.preview_json.risk_flags.map((flag) => (
                <Badge
                  className='bg-destructive/10 text-destructive'
                  key={flag}
                  variant='secondary'
                >
                  {flag}
                </Badge>
              ))}
            </div>
          ) : null}
          <pre className='max-h-48 overflow-auto rounded-xl bg-muted/70 p-3 text-[11px] leading-5 text-foreground/85'>
            {approval.preview_json.command ??
              approval.preview_json.description ??
              'No command preview'}
          </pre>
        </div>
      ) : (
        <pre className='mt-3 max-h-48 overflow-auto rounded-xl bg-muted/70 p-3 text-[11px] leading-5 text-foreground/85'>
          {approval.preview_json.diff ?? 'No diff preview'}
        </pre>
      )}

      {approval.status === 'pending' && onResolveApproval ? (
        <div className='mt-3 flex gap-2'>
          <Button onClick={() => onResolveApproval(approval.id, true)}>允许</Button>
          <Button onClick={() => onResolveApproval(approval.id, false)} variant='secondary'>
            拒绝
          </Button>
        </div>
      ) : (
        <p className='mt-3 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground'>
          {approval.status}
        </p>
      )}
    </Card>
  )
}
