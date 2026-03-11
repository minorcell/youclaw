import {
  BarChart3,
  Clock3,
  Database,
  ListFilter,
  Loader2,
  Wrench,
} from "lucide-react"
import { useEffect, useMemo, useRef, useState } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useToastContext } from "@/contexts/toast-context"
import { getAppClient } from "@/lib/app-client"
import { flattenProviderProfiles } from "@/lib/provider-profiles"
import type {
  ProviderAccount,
  UsageLogDetailPayload,
  UsageLogsPayload,
  UsageModelStatsPayload,
  UsageProviderStatsPayload,
  UsageSettingsPayload,
  UsageStatsRange,
  UsageSummaryPayload,
  UsageToolStatsPayload,
} from "@/lib/types"
import { cn } from "@/lib/utils"

interface UsageSettingsSectionProps {
  providerAccounts: ProviderAccount[]
}

type UsageTab = "logs" | "providers" | "models" | "tools"

type DetailFilter = "all" | "on" | "off"

const DEFAULT_PAGE_SIZE = 20

const rangeOptions: Array<{ value: UsageStatsRange; label: string }> = [
  { value: "24h", label: "24h" },
  { value: "7d", label: "7天" },
  { value: "30d", label: "30天" },
  { value: "all", label: "全部" },
]

const statusOptions: Array<{ value: string; label: string }> = [
  { value: "all", label: "全部状态" },
  { value: "running", label: "运行中" },
  { value: "completed", label: "已完成" },
  { value: "failed", label: "失败" },
  { value: "cancelled", label: "已取消" },
]

const detailFilterOptions: Array<{ value: DetailFilter; label: string }> = [
  { value: "all", label: "详情记录：全部" },
  { value: "on", label: "详情记录：开启" },
  { value: "off", label: "详情记录：关闭" },
]

function formatNumber(value: number): string {
  return new Intl.NumberFormat("zh-CN").format(Math.max(0, value))
}

function formatDateTime(value: string | null): string {
  if (!value) return "-"
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleString("zh-CN", {
    hour12: false,
  })
}

function formatDuration(value: number | null): string {
  if (value === null || value < 0) return "-"
  if (value < 1000) return `${value} ms`
  return `${(value / 1000).toFixed(2)} s`
}

function statusLabel(status: string): string {
  switch (status) {
    case "running":
      return "运行中"
    case "completed":
      return "已完成"
    case "failed":
      return "失败"
    case "cancelled":
      return "已取消"
    default:
      return status
  }
}

function statusBadgeClass(status: string): string {
  if (status === "completed") {
    return "bg-emerald-500/10 text-emerald-700"
  }
  if (status === "failed") {
    return "bg-destructive/10 text-destructive"
  }
  if (status === "cancelled") {
    return "bg-amber-500/10 text-amber-700"
  }
  return "bg-background text-foreground"
}

function errorMessageFromUnknown(error: unknown): string {
  if (typeof error === "string") {
    return error
  }
  if (error instanceof Error) {
    return error.message
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message
  }
  return "操作失败，请稍后重试。"
}

function SummaryItem({
  label,
  value,
  hint,
}: {
  label: string
  value: string
  hint?: string
}) {
  return (
    <div className="rounded-xl bg-background/80 p-3">
      <p className="text-xs uppercase tracking-[0.16em] text-muted-foreground">
        {label}
      </p>
      <p className="mt-2 text-xl font-semibold tracking-tight">{value}</p>
      {hint ? (
        <p className="mt-1 text-xs text-muted-foreground">{hint}</p>
      ) : null}
    </div>
  )
}

function PaginationBar({
  loading,
  page,
  total,
  hasMore,
  onPrev,
  onNext,
}: {
  loading: boolean
  page: number
  total: number
  hasMore: boolean
  onPrev: () => void
  onNext: () => void
}) {
  return (
    <div className="flex items-center justify-end gap-2 pt-3">
      <p className="mr-auto text-xs text-muted-foreground">
        共 {formatNumber(total)} 条
      </p>
      <Button
        disabled={loading || page <= 1}
        onClick={onPrev}
        size="sm"
        type="button"
        variant="outline"
      >
        上一页
      </Button>
      <span className="text-xs text-muted-foreground">第 {page} 页</span>
      <Button
        disabled={loading || !hasMore}
        onClick={onNext}
        size="sm"
        type="button"
        variant="outline"
      >
        下一页
      </Button>
    </div>
  )
}

export function UsageSettingsSection({
  providerAccounts,
}: UsageSettingsSectionProps) {
  const { error: toastError, success: toastSuccess } = useToastContext()

  const providers = useMemo(
    () => flattenProviderProfiles(providerAccounts),
    [providerAccounts],
  )

  const [range, setRange] = useState<UsageStatsRange>("7d")
  const [activeTab, setActiveTab] = useState<UsageTab>("logs")

  const [summary, setSummary] = useState<UsageSummaryPayload | null>(null)
  const [summaryLoading, setSummaryLoading] = useState(false)

  const [usageSettings, setUsageSettings] =
    useState<UsageSettingsPayload | null>(null)
  const [settingsBusy, setSettingsBusy] = useState(false)

  const [logsPage, setLogsPage] = useState(1)
  const [logsLoading, setLogsLoading] = useState(false)
  const [logsData, setLogsData] = useState<UsageLogsPayload | null>(null)
  const [logModelId, setLogModelId] = useState("all")
  const [logStatus, setLogStatus] = useState("all")
  const [detailFilter, setDetailFilter] = useState<DetailFilter>("all")

  const [providersPage, setProvidersPage] = useState(1)
  const [providersLoading, setProvidersLoading] = useState(false)
  const [providersData, setProvidersData] =
    useState<UsageProviderStatsPayload | null>(null)

  const [modelsPage, setModelsPage] = useState(1)
  const [modelsLoading, setModelsLoading] = useState(false)
  const [modelsData, setModelsData] = useState<UsageModelStatsPayload | null>(
    null,
  )

  const [toolsPage, setToolsPage] = useState(1)
  const [toolsLoading, setToolsLoading] = useState(false)
  const [toolsData, setToolsData] = useState<UsageToolStatsPayload | null>(null)

  const [expandedTurnId, setExpandedTurnId] = useState<string | null>(null)
  const [detailLoadingTurnId, setDetailLoadingTurnId] = useState<string | null>(
    null,
  )
  const [detailsByTurnId, setDetailsByTurnId] = useState<
    Record<string, UsageLogDetailPayload>
  >({})

  const summaryRequestIdRef = useRef(0)
  const logsRequestIdRef = useRef(0)
  const providersRequestIdRef = useRef(0)
  const modelsRequestIdRef = useRef(0)
  const toolsRequestIdRef = useRef(0)

  const modelOptions = useMemo(
    () =>
      providers.map((provider) => ({
        id: provider.id,
        label: `${provider.name} / ${provider.model_name || provider.model}`,
      })),
    [providers],
  )

  useEffect(() => {
    let disposed = false

    async function fetchUsageSettings() {
      try {
        const payload = await getAppClient().request<UsageSettingsPayload>(
          "usage.settings.get",
          {},
        )
        if (!disposed) {
          setUsageSettings(payload)
        }
      } catch (error) {
        if (!disposed) {
          toastError(errorMessageFromUnknown(error))
        }
      }
    }

    void fetchUsageSettings()

    return () => {
      disposed = true
    }
  }, [toastError])

  useEffect(() => {
    const requestId = ++summaryRequestIdRef.current
    let disposed = false
    setSummaryLoading(true)

    async function fetchSummary() {
      try {
        const payload = await getAppClient().request<UsageSummaryPayload>(
          "usage.summary.get",
          {
            range,
          },
        )
        if (!disposed && requestId === summaryRequestIdRef.current) {
          setSummary(payload)
        }
      } catch (error) {
        if (!disposed && requestId === summaryRequestIdRef.current) {
          toastError(errorMessageFromUnknown(error))
        }
      } finally {
        if (!disposed && requestId === summaryRequestIdRef.current) {
          setSummaryLoading(false)
        }
      }
    }

    void fetchSummary()

    return () => {
      disposed = true
    }
  }, [range, toastError])

  useEffect(() => {
    setLogsPage(1)
    setProvidersPage(1)
    setModelsPage(1)
    setToolsPage(1)
  }, [range])

  useEffect(() => {
    setLogsPage(1)
  }, [logModelId, logStatus, detailFilter])

  useEffect(() => {
    if (activeTab !== "logs") return
    const requestId = ++logsRequestIdRef.current
    let disposed = false
    setLogsLoading(true)

    async function fetchLogs() {
      try {
        const payload = await getAppClient().request<UsageLogsPayload>(
          "usage.logs.list",
          {
            range,
            provider_profile_id: logModelId === "all" ? null : logModelId,
            status: logStatus === "all" ? null : logStatus,
            detail_logged:
              detailFilter === "all" ? null : detailFilter === "on",
            page: logsPage,
            page_size: DEFAULT_PAGE_SIZE,
          },
        )
        if (!disposed && requestId === logsRequestIdRef.current) {
          setLogsData(payload)
        }
      } catch (error) {
        if (!disposed && requestId === logsRequestIdRef.current) {
          toastError(errorMessageFromUnknown(error))
        }
      } finally {
        if (!disposed && requestId === logsRequestIdRef.current) {
          setLogsLoading(false)
        }
      }
    }

    void fetchLogs()

    return () => {
      disposed = true
    }
  }, [
    activeTab,
    range,
    logModelId,
    logStatus,
    detailFilter,
    logsPage,
    toastError,
  ])

  useEffect(() => {
    if (activeTab !== "providers") return
    const requestId = ++providersRequestIdRef.current
    let disposed = false
    setProvidersLoading(true)

    async function fetchProviderStats() {
      try {
        const payload = await getAppClient().request<UsageProviderStatsPayload>(
          "usage.stats.providers.list",
          {
            range,
            page: providersPage,
            page_size: DEFAULT_PAGE_SIZE,
          },
        )
        if (!disposed && requestId === providersRequestIdRef.current) {
          setProvidersData(payload)
        }
      } catch (error) {
        if (!disposed && requestId === providersRequestIdRef.current) {
          toastError(errorMessageFromUnknown(error))
        }
      } finally {
        if (!disposed && requestId === providersRequestIdRef.current) {
          setProvidersLoading(false)
        }
      }
    }

    void fetchProviderStats()

    return () => {
      disposed = true
    }
  }, [activeTab, range, providersPage, toastError])

  useEffect(() => {
    if (activeTab !== "models") return
    const requestId = ++modelsRequestIdRef.current
    let disposed = false
    setModelsLoading(true)

    async function fetchModelStats() {
      try {
        const payload = await getAppClient().request<UsageModelStatsPayload>(
          "usage.stats.models.list",
          {
            range,
            page: modelsPage,
            page_size: DEFAULT_PAGE_SIZE,
          },
        )
        if (!disposed && requestId === modelsRequestIdRef.current) {
          setModelsData(payload)
        }
      } catch (error) {
        if (!disposed && requestId === modelsRequestIdRef.current) {
          toastError(errorMessageFromUnknown(error))
        }
      } finally {
        if (!disposed && requestId === modelsRequestIdRef.current) {
          setModelsLoading(false)
        }
      }
    }

    void fetchModelStats()

    return () => {
      disposed = true
    }
  }, [activeTab, range, modelsPage, toastError])

  useEffect(() => {
    if (activeTab !== "tools") return
    const requestId = ++toolsRequestIdRef.current
    let disposed = false
    setToolsLoading(true)

    async function fetchToolStats() {
      try {
        const payload = await getAppClient().request<UsageToolStatsPayload>(
          "usage.stats.tools.list",
          {
            range,
            page: toolsPage,
            page_size: DEFAULT_PAGE_SIZE,
          },
        )
        if (!disposed && requestId === toolsRequestIdRef.current) {
          setToolsData(payload)
        }
      } catch (error) {
        if (!disposed && requestId === toolsRequestIdRef.current) {
          toastError(errorMessageFromUnknown(error))
        }
      } finally {
        if (!disposed && requestId === toolsRequestIdRef.current) {
          setToolsLoading(false)
        }
      }
    }

    void fetchToolStats()

    return () => {
      disposed = true
    }
  }, [activeTab, range, toolsPage, toastError])

  async function handleDetailLoggingChange(checked: boolean) {
    setSettingsBusy(true)
    try {
      const payload = await getAppClient().request<UsageSettingsPayload>(
        "usage.settings.update",
        {
          detail_logging_enabled: checked,
        },
      )
      setUsageSettings(payload)
      toastSuccess(checked ? "已开启详情记录。" : "已关闭详情记录。")
    } catch (error) {
      toastError(errorMessageFromUnknown(error))
    } finally {
      setSettingsBusy(false)
    }
  }

  async function handleToggleTurnDetail(turnId: string) {
    if (expandedTurnId === turnId) {
      setExpandedTurnId(null)
      return
    }

    setExpandedTurnId(turnId)
    if (detailsByTurnId[turnId]) {
      return
    }

    setDetailLoadingTurnId(turnId)
    try {
      const payload = await getAppClient().request<UsageLogDetailPayload>(
        "usage.logs.detail",
        {
          turn_id: turnId,
        },
      )
      setDetailsByTurnId((current) => ({
        ...current,
        [turnId]: payload,
      }))
    } catch (error) {
      toastError(errorMessageFromUnknown(error))
    } finally {
      setDetailLoadingTurnId(null)
    }
  }

  return (
    <div className="space-y-4">
      <Card className="bg-card/80 py-0 shadow-none">
        <CardHeader className="space-y-4 py-4">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <CardTitle>使用概览</CardTitle>
              <CardDescription>
                按时间范围查看 Turn 与 Token 消耗。
              </CardDescription>
            </div>
            <div className="flex items-center gap-2 rounded-xl bg-background/80 p-1">
              {rangeOptions.map((item) => (
                <Button
                  className={cn(
                    "h-8 rounded-lg px-3",
                    range === item.value && "shadow-none",
                  )}
                  key={item.value}
                  onClick={() => setRange(item.value)}
                  size="sm"
                  type="button"
                  variant={range === item.value ? "default" : "ghost"}
                >
                  {item.label}
                </Button>
              ))}
            </div>
          </div>
        </CardHeader>

        <CardContent className="grid gap-3 py-4 sm:grid-cols-2 xl:grid-cols-4">
          <SummaryItem
            label="总 Turn"
            value={
              summaryLoading ? "..." : formatNumber(summary?.total_turns ?? 0)
            }
            hint={`总 Step ${formatNumber(summary?.total_steps ?? 0)}`}
          />
          <SummaryItem
            label="输入 Token"
            value={
              summaryLoading ? "..." : formatNumber(summary?.input_tokens ?? 0)
            }
            hint={`缓存读取 ${formatNumber(summary?.input_cache_read_tokens ?? 0)}`}
          />
          <SummaryItem
            label="输出 Token"
            value={
              summaryLoading ? "..." : formatNumber(summary?.output_tokens ?? 0)
            }
            hint={`推理 ${formatNumber(summary?.reasoning_tokens ?? 0)}`}
          />
          <SummaryItem
            label="总 Token"
            value={
              summaryLoading ? "..." : formatNumber(summary?.total_tokens ?? 0)
            }
            hint={`平均步数 ${((summary?.avg_steps_per_turn ?? 0) || 0).toFixed(2)}`}
          />
        </CardContent>
      </Card>

      <Card className="bg-card/80 py-0 shadow-none">
        <CardContent className="space-y-4 py-4">
          <Tabs
            onValueChange={(value) => {
              if (
                value === "logs" ||
                value === "providers" ||
                value === "models" ||
                value === "tools"
              ) {
                setActiveTab(value)
              }
            }}
            value={activeTab}
          >
            <TabsList className="grid w-full grid-cols-4" variant="default">
              <TabsTrigger value="logs">
                <ListFilter className="h-4 w-4" /> Turn 日志
              </TabsTrigger>
              <TabsTrigger value="providers">
                <Database className="h-4 w-4" /> 供应商统计
              </TabsTrigger>
              <TabsTrigger value="models">
                <BarChart3 className="h-4 w-4" /> 模型统计
              </TabsTrigger>
              <TabsTrigger value="tools">
                <Wrench className="h-4 w-4" /> 工具统计
              </TabsTrigger>
            </TabsList>

            <TabsContent className="pt-3" value="logs">
              <div className="mb-3 grid gap-2 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(0,1fr)_auto]">
                <Select
                  onValueChange={(value) => setLogModelId(value ?? "all")}
                  value={logModelId}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder="选择模型" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">全部模型</SelectItem>
                    {modelOptions.map((model) => (
                      <SelectItem key={model.id} value={model.id}>
                        {model.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>

                <Select
                  onValueChange={(value) => setLogStatus(value ?? "all")}
                  value={logStatus}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder="选择状态" />
                  </SelectTrigger>
                  <SelectContent>
                    {statusOptions.map((item) => (
                      <SelectItem key={item.value} value={item.value}>
                        {item.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>

                <Select
                  onValueChange={(value) => {
                    if (value === "on" || value === "off" || value === "all") {
                      setDetailFilter(value)
                      return
                    }
                    setDetailFilter("all")
                  }}
                  value={detailFilter}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder="详情记录筛选" />
                  </SelectTrigger>
                  <SelectContent>
                    {detailFilterOptions.map((item) => (
                      <SelectItem key={item.value} value={item.value}>
                        {item.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>

                <label className="flex items-center justify-end gap-2 rounded-xl bg-background/80 px-3 py-2 text-sm">
                  详情记录开关
                  <Switch
                    checked={usageSettings?.detail_logging_enabled ?? false}
                    disabled={settingsBusy}
                    onCheckedChange={handleDetailLoggingChange}
                  />
                </label>
              </div>

              <div className="space-y-2">
                {logsLoading ? (
                  <div className="flex items-center gap-2 rounded-xl px-3 py-5 text-sm text-muted-foreground">
                    <Loader2 className="h-4 w-4 animate-spin" /> 加载 Turn
                    日志中...
                  </div>
                ) : logsData?.items.length ? (
                  logsData.items.map((item) => {
                    const isExpanded = expandedTurnId === item.turn_id
                    const detailPayload = detailsByTurnId[item.turn_id]
                    return (
                      <div
                        className="rounded-xl bbg-background/75 p-3"
                        key={item.turn_id}
                      >
                        <div className="flex flex-wrap items-center gap-2">
                          <Badge
                            className={cn("", statusBadgeClass(item.status))}
                          >
                            {statusLabel(item.status)}
                          </Badge>
                          <Badge className="bg-card text-foreground">
                            {item.provider_name ?? "未绑定服务商"}
                          </Badge>
                          <Badge className="bg-card text-foreground">
                            {item.model_name ?? item.model ?? "未绑定模型"}
                          </Badge>
                          <Badge className="bg-card text-foreground">
                            详情{item.detail_logged ? "开启" : "关闭"}
                          </Badge>
                        </div>

                        <p className="mt-2 line-clamp-2 text-sm text-foreground/90">
                          {item.user_message || "(空 Turn)"}
                        </p>

                        <div className="mt-2 flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
                          <span>{formatDateTime(item.started_at)}</span>
                          <span>耗时: {formatDuration(item.duration_ms)}</span>
                          <span>steps: {formatNumber(item.step_count)}</span>
                          <span>
                            tokens: {formatNumber(item.input_tokens)}/
                            {formatNumber(item.output_tokens)}/
                            {formatNumber(item.total_tokens)}
                          </span>
                          <span>
                            缓存读: {formatNumber(item.input_cache_read_tokens)}
                          </span>
                        </div>

                        <div className="mt-3">
                          <Button
                            disabled={detailLoadingTurnId === item.turn_id}
                            onClick={() =>
                              void handleToggleTurnDetail(item.turn_id)
                            }
                            size="sm"
                            type="button"
                            variant="outline"
                          >
                            {detailLoadingTurnId === item.turn_id ? (
                              <Loader2 className="mr-1 h-4 w-4 animate-spin" />
                            ) : (
                              <Clock3 className="mr-1 h-4 w-4" />
                            )}
                            {isExpanded ? "收起详情" : "查看详情"}
                          </Button>
                        </div>

                        {isExpanded ? (
                          <div className="mt-3 space-y-2 rounded-xl bg-muted/35 p-2">
                            {detailPayload?.tools.length ? (
                              detailPayload.tools.map((tool) => (
                                <div
                                  className="rounded-lg bg-background/80 px-3 py-2"
                                  key={tool.id}
                                >
                                  <div className="flex flex-wrap items-center gap-2 text-xs">
                                    <Badge className="bg-card text-foreground">
                                      {tool.tool_name}
                                    </Badge>
                                    {tool.tool_action ? (
                                      <Badge className="bg-card text-foreground">
                                        {tool.tool_action}
                                      </Badge>
                                    ) : null}
                                    <Badge
                                      className={cn(
                                        "",
                                        tool.is_error
                                          ? "bg-destructive/10 text-destructive"
                                          : "bg-emerald-500/10 text-emerald-700",
                                      )}
                                    >
                                      {tool.status}
                                    </Badge>
                                    <span className="text-muted-foreground">
                                      {formatDuration(tool.duration_ms)}
                                    </span>
                                    <span className="text-muted-foreground">
                                      {formatDateTime(tool.created_at)}
                                    </span>
                                  </div>
                                </div>
                              ))
                            ) : (
                              <p className="px-1 py-2 text-xs text-muted-foreground">
                                当前 Turn 没有可展示的工具详情。
                              </p>
                            )}
                          </div>
                        ) : null}
                      </div>
                    )
                  })
                ) : (
                  <div className="rounded-xl px-3 py-5 text-sm text-muted-foreground">
                    当前筛选下暂无 Turn 日志。
                  </div>
                )}
              </div>

              <PaginationBar
                hasMore={logsData?.page.has_more ?? false}
                loading={logsLoading}
                onNext={() => setLogsPage((current) => current + 1)}
                onPrev={() =>
                  setLogsPage((current) => Math.max(1, current - 1))
                }
                page={logsData?.page.page ?? logsPage}
                total={logsData?.page.total ?? 0}
              />
            </TabsContent>

            <TabsContent className="pt-3" value="providers">
              <div className="space-y-2">
                {providersLoading ? (
                  <div className="flex items-center gap-2 rounded-xl  px-3 py-5 text-sm text-muted-foreground">
                    <Loader2 className="h-4 w-4 animate-spin" />{" "}
                    加载供应商统计中...
                  </div>
                ) : providersData?.items.length ? (
                  providersData.items.map((item, index) => (
                    <div
                      className="grid gap-2 rounded-xl  bg-background/75 p-3 md:grid-cols-[32px_minmax(0,1fr)_repeat(4,minmax(0,1fr))] md:items-center"
                      key={`${item.provider_id ?? "unknown"}-${index}`}
                    >
                      <p className="text-sm font-semibold text-muted-foreground">
                        #{index + 1}
                      </p>
                      <p className="truncate text-sm font-medium">
                        {item.provider_name ?? "未识别服务商"}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        Turn {formatNumber(item.turn_count)}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        成功 {formatNumber(item.completed_count)}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        失败 {formatNumber(item.failed_count)}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        Token {formatNumber(item.total_tokens)}
                      </p>
                    </div>
                  ))
                ) : (
                  <div className="rounded-xl px-3 py-5 text-sm text-muted-foreground">
                    当前范围暂无供应商统计。
                  </div>
                )}
              </div>

              <PaginationBar
                hasMore={providersData?.page.has_more ?? false}
                loading={providersLoading}
                onNext={() => setProvidersPage((current) => current + 1)}
                onPrev={() =>
                  setProvidersPage((current) => Math.max(1, current - 1))
                }
                page={providersData?.page.page ?? providersPage}
                total={providersData?.page.total ?? 0}
              />
            </TabsContent>

            <TabsContent className="pt-3" value="models">
              <div className="space-y-2">
                {modelsLoading ? (
                  <div className="flex items-center gap-2 rounded-xl  px-3 py-5 text-sm text-muted-foreground">
                    <Loader2 className="h-4 w-4 animate-spin" />{" "}
                    加载模型统计中...
                  </div>
                ) : modelsData?.items.length ? (
                  modelsData.items.map((item, index) => (
                    <div
                      className="grid gap-2 rounded-xl  bg-background/75 p-3 md:grid-cols-[32px_minmax(0,1fr)_repeat(4,minmax(0,1fr))] md:items-center"
                      key={`${item.model_id ?? "unknown"}-${index}`}
                    >
                      <p className="text-sm font-semibold text-muted-foreground">
                        #{index + 1}
                      </p>
                      <div className="min-w-0">
                        <p className="truncate text-sm font-medium">
                          {item.model_name ?? item.model ?? "未识别模型"}
                        </p>
                        <p className="truncate text-xs text-muted-foreground">
                          {item.provider_name ?? "未识别服务商"}
                        </p>
                      </div>
                      <p className="text-xs text-muted-foreground">
                        Turn {formatNumber(item.turn_count)}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        成功 {formatNumber(item.completed_count)}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        Token {formatNumber(item.total_tokens)}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        均耗时 {formatDuration(item.avg_duration_ms)}
                      </p>
                    </div>
                  ))
                ) : (
                  <div className="rounded-xl px-3 py-5 text-sm text-muted-foreground">
                    当前范围暂无模型统计。
                  </div>
                )}
              </div>

              <PaginationBar
                hasMore={modelsData?.page.has_more ?? false}
                loading={modelsLoading}
                onNext={() => setModelsPage((current) => current + 1)}
                onPrev={() =>
                  setModelsPage((current) => Math.max(1, current - 1))
                }
                page={modelsData?.page.page ?? modelsPage}
                total={modelsData?.page.total ?? 0}
              />
            </TabsContent>

            <TabsContent className="pt-3" value="tools">
              <div className="space-y-2">
                {toolsLoading ? (
                  <div className="flex items-center gap-2 rounded-xl  px-3 py-5 text-sm text-muted-foreground">
                    <Loader2 className="h-4 w-4 animate-spin" />{" "}
                    加载工具统计中...
                  </div>
                ) : toolsData?.items.length ? (
                  toolsData.items.map((item, index) => (
                    <div
                      className="grid gap-2 rounded-xl  bg-background/75 p-3 md:grid-cols-[32px_minmax(0,1fr)_repeat(4,minmax(0,1fr))] md:items-center"
                      key={`${item.tool_name}-${item.tool_action ?? "all"}-${index}`}
                    >
                      <p className="text-sm font-semibold text-muted-foreground">
                        #{index + 1}
                      </p>
                      <div className="min-w-0">
                        <p className="truncate text-sm font-medium">
                          {item.tool_name}
                        </p>
                        <p className="truncate text-xs text-muted-foreground">
                          {item.tool_action ?? "(无动作标识)"}
                        </p>
                      </div>
                      <p className="text-xs text-muted-foreground">
                        调用 {formatNumber(item.call_count)}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        成功 {formatNumber(item.success_count)}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        错误 {formatNumber(item.error_count)}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        均耗时 {formatDuration(item.avg_duration_ms)}
                      </p>
                    </div>
                  ))
                ) : (
                  <div className="rounded-xl px-3 py-5 text-sm text-muted-foreground">
                    当前范围暂无工具统计。
                  </div>
                )}
              </div>

              <PaginationBar
                hasMore={toolsData?.page.has_more ?? false}
                loading={toolsLoading}
                onNext={() => setToolsPage((current) => current + 1)}
                onPrev={() =>
                  setToolsPage((current) => Math.max(1, current - 1))
                }
                page={toolsData?.page.page ?? toolsPage}
                total={toolsData?.page.total ?? 0}
              />
            </TabsContent>
          </Tabs>
        </CardContent>
      </Card>
    </div>
  )
}
