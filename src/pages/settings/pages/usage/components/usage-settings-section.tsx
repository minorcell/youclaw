import { BarChart3, Database, ListFilter, Wrench } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'

import { Card, CardContent } from '@/components/ui/card'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { useToastContext } from '@/contexts/toast-context'
import { getAppClient } from '@/lib/app-client'
import { flattenProviderProfiles } from '@/lib/provider-profiles'
import type {
  ProviderAccount,
  UsageLogDetailPayload,
  UsageLogItem,
  UsageLogsPayload,
  UsageModelStatsPayload,
  UsageProviderStatsPayload,
  UsageSummaryPayload,
  UsageToolStatsPayload,
} from '@/lib/types'
import {
  SETTINGS_CARD_CLASSNAME,
  SETTINGS_CARD_CONTENT_CLASSNAME,
} from '@/pages/settings/lib/ui'

import { UsageLogDetailDialog } from './usage-log-detail-dialog'
import { UsageLogsTab } from './usage-logs-tab'
import { UsageModelsTab } from './usage-models-tab'
import { UsageProvidersTab } from './usage-providers-tab'
import { UsageSummaryCard } from './usage-summary-card'
import {
  DEFAULT_PAGE_SIZE,
  type UsageTab,
  type UsageModelOption,
  errorMessageFromUnknown,
} from './usage-shared'
import { UsageToolsTab } from './usage-tools-tab'

interface UsageSettingsSectionProps {
  providerAccounts: ProviderAccount[]
}

export function UsageSettingsSection({ providerAccounts }: UsageSettingsSectionProps) {
  const { error: toastError } = useToastContext()

  const providers = useMemo(() => flattenProviderProfiles(providerAccounts), [providerAccounts])
  const modelOptions = useMemo<UsageModelOption[]>(
    () =>
      providers.map((provider) => ({
        id: provider.id,
        label: `${provider.name} / ${provider.model_name || provider.model}`,
      })),
    [providers],
  )

  const [range, setRange] = useState<'24h' | '7d' | '30d' | 'all'>('7d')
  const [activeTab, setActiveTab] = useState<UsageTab>('logs')

  const [summary, setSummary] = useState<UsageSummaryPayload | null>(null)
  const [summaryLoading, setSummaryLoading] = useState(false)

  const [logsPage, setLogsPage] = useState(1)
  const [logsLoading, setLogsLoading] = useState(false)
  const [logsData, setLogsData] = useState<UsageLogsPayload | null>(null)
  const [logModelId, setLogModelId] = useState('all')
  const [logStatus, setLogStatus] = useState('all')

  const [providersPage, setProvidersPage] = useState(1)
  const [providersLoading, setProvidersLoading] = useState(false)
  const [providersData, setProvidersData] = useState<UsageProviderStatsPayload | null>(null)

  const [modelsPage, setModelsPage] = useState(1)
  const [modelsLoading, setModelsLoading] = useState(false)
  const [modelsData, setModelsData] = useState<UsageModelStatsPayload | null>(null)

  const [toolsPage, setToolsPage] = useState(1)
  const [toolsLoading, setToolsLoading] = useState(false)
  const [toolsData, setToolsData] = useState<UsageToolStatsPayload | null>(null)

  const [selectedLogItem, setSelectedLogItem] = useState<UsageLogItem | null>(null)
  const [detailLoadingTurnId, setDetailLoadingTurnId] = useState<string | null>(null)
  const [detailsByTurnId, setDetailsByTurnId] = useState<Record<string, UsageLogDetailPayload>>({})

  const summaryRequestIdRef = useRef(0)
  const logsRequestIdRef = useRef(0)
  const providersRequestIdRef = useRef(0)
  const modelsRequestIdRef = useRef(0)
  const toolsRequestIdRef = useRef(0)

  useEffect(() => {
    const requestId = ++summaryRequestIdRef.current
    let disposed = false
    setSummaryLoading(true)

    async function fetchSummary() {
      try {
        const payload = await getAppClient().request<UsageSummaryPayload>('usage.summary.get', {
          range,
        })
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
  }, [logModelId, logStatus])

  useEffect(() => {
    if (activeTab !== 'logs') return
    const requestId = ++logsRequestIdRef.current
    let disposed = false
    setLogsLoading(true)

    async function fetchLogs() {
      try {
        const payload = await getAppClient().request<UsageLogsPayload>('usage.logs.list', {
          range,
          provider_profile_id: logModelId === 'all' ? null : logModelId,
          status: logStatus === 'all' ? null : logStatus,
          page: logsPage,
          page_size: DEFAULT_PAGE_SIZE,
        })
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
  }, [activeTab, range, logModelId, logStatus, logsPage, toastError])

  useEffect(() => {
    if (activeTab !== 'providers') return
    const requestId = ++providersRequestIdRef.current
    let disposed = false
    setProvidersLoading(true)

    async function fetchProviderStats() {
      try {
        const payload = await getAppClient().request<UsageProviderStatsPayload>(
          'usage.stats.providers.list',
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
    if (activeTab !== 'models') return
    const requestId = ++modelsRequestIdRef.current
    let disposed = false
    setModelsLoading(true)

    async function fetchModelStats() {
      try {
        const payload = await getAppClient().request<UsageModelStatsPayload>(
          'usage.stats.models.list',
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
    if (activeTab !== 'tools') return
    const requestId = ++toolsRequestIdRef.current
    let disposed = false
    setToolsLoading(true)

    async function fetchToolStats() {
      try {
        const payload = await getAppClient().request<UsageToolStatsPayload>(
          'usage.stats.tools.list',
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

  async function handleOpenLogDetail(item: UsageLogItem) {
    setSelectedLogItem(item)
    if (detailsByTurnId[item.turn_id]) {
      return
    }

    setDetailLoadingTurnId(item.turn_id)
    try {
      const payload = await getAppClient().request<UsageLogDetailPayload>('usage.logs.detail', {
        turn_id: item.turn_id,
      })
      setDetailsByTurnId((current) => ({
        ...current,
        [item.turn_id]: payload,
      }))
    } catch (error) {
      toastError(errorMessageFromUnknown(error))
    } finally {
      setDetailLoadingTurnId((current) => (current === item.turn_id ? null : current))
    }
  }

  return (
    <div className='space-y-4'>
      <UsageSummaryCard
        onRangeChange={setRange}
        range={range}
        summary={summary}
        summaryLoading={summaryLoading}
      />

      <Card className={SETTINGS_CARD_CLASSNAME}>
        <CardContent className={`${SETTINGS_CARD_CONTENT_CLASSNAME} py-4`}>
          <Tabs
            onValueChange={(value) => {
              if (
                value === 'logs' ||
                value === 'providers' ||
                value === 'models' ||
                value === 'tools'
              ) {
                setActiveTab(value)
              }
            }}
            value={activeTab}
          >
            <TabsList className='grid w-full grid-cols-4' variant='default'>
              <TabsTrigger value='logs'>
                <ListFilter className='h-4 w-4' /> Turn 日志
              </TabsTrigger>
              <TabsTrigger value='providers'>
                <Database className='h-4 w-4' /> 供应商统计
              </TabsTrigger>
              <TabsTrigger value='models'>
                <BarChart3 className='h-4 w-4' /> 模型统计
              </TabsTrigger>
              <TabsTrigger value='tools'>
                <Wrench className='h-4 w-4' /> 工具统计
              </TabsTrigger>
            </TabsList>

            <TabsContent value='logs'>
              <UsageLogsTab
                currentPage={logsPage}
                detailLoadingTurnId={detailLoadingTurnId}
                loading={logsLoading}
                logModelId={logModelId}
                logStatus={logStatus}
                logsData={logsData}
                modelOptions={modelOptions}
                onLogModelIdChange={setLogModelId}
                onLogStatusChange={setLogStatus}
                onNextPage={() => setLogsPage((current) => current + 1)}
                onOpenDetail={(item) => void handleOpenLogDetail(item)}
                onPrevPage={() => setLogsPage((current) => Math.max(1, current - 1))}
              />
            </TabsContent>

            <TabsContent value='providers'>
              <UsageProvidersTab
                data={providersData}
                loading={providersLoading}
                onNextPage={() => setProvidersPage((current) => current + 1)}
                onPrevPage={() => setProvidersPage((current) => Math.max(1, current - 1))}
                page={providersPage}
              />
            </TabsContent>

            <TabsContent value='models'>
              <UsageModelsTab
                data={modelsData}
                loading={modelsLoading}
                onNextPage={() => setModelsPage((current) => current + 1)}
                onPrevPage={() => setModelsPage((current) => Math.max(1, current - 1))}
                page={modelsPage}
              />
            </TabsContent>

            <TabsContent value='tools'>
              <UsageToolsTab
                data={toolsData}
                loading={toolsLoading}
                onNextPage={() => setToolsPage((current) => current + 1)}
                onPrevPage={() => setToolsPage((current) => Math.max(1, current - 1))}
                page={toolsPage}
              />
            </TabsContent>
          </Tabs>
        </CardContent>
      </Card>

      <UsageLogDetailDialog
        detail={selectedLogItem ? detailsByTurnId[selectedLogItem.turn_id] ?? null : null}
        item={selectedLogItem}
        loading={selectedLogItem ? detailLoadingTurnId === selectedLogItem.turn_id : false}
        onOpenChange={(open) => {
          if (!open) {
            setSelectedLogItem(null)
          }
        }}
        open={selectedLogItem !== null}
      />
    </div>
  )
}
