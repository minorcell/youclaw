import type { SessionApprovalMode } from '@/lib/types'

export const sessionApprovalModeOptions: Array<{
  value: SessionApprovalMode
  label: string
  description: string
}> = [
  {
    value: 'default',
    label: '默认权限',
    description: '写入前确认',
  },
  {
    value: 'full_access',
    label: '完整权限',
    description: '工具直接执行',
  },
]

export function labelForSessionApprovalMode(value: SessionApprovalMode): string {
  return sessionApprovalModeOptions.find((item) => item.value === value)?.label ?? '默认权限'
}
