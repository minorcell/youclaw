import { useEffect, useMemo, useState } from 'react'

import { useToastContext } from '@/contexts/toast-context'
import { getAppClient } from '@/lib/app-client'
import type { ProviderAccount, ProviderModel } from '@/lib/types'
import { ProviderSettingsSection } from '@/pages/settings/pages/providers/components/provider-settings-section'
import { useAppStore } from '@/store/app-store'
import { useSettingsStore } from '@/store/settings-store'

function errorMessageFromUnknown(error: unknown): string {
  if (typeof error === 'string') {
    return error
  }
  if (error instanceof Error) {
    return error.message
  }
  if (
    typeof error === 'object' &&
    error !== null &&
    'message' in error &&
    typeof error.message === 'string'
  ) {
    return error.message
  }
  return '操作失败，请稍后重试。'
}

export function ProvidersSettingsPage() {
  const providerAccounts = useAppStore((state) => state.providerAccounts)
  const selectedProviderId = useSettingsStore((state) => state.selectedProviderId)
  const setSelectedProviderId = useSettingsStore((state) => state.setSelectedProviderId)
  const { success: toastSuccess, error: toastError } = useToastContext()
  const [accountBusy, setAccountBusy] = useState(false)
  const [modelBusyId, setModelBusyId] = useState<string | null>(null)

  useEffect(() => {
    if (providerAccounts.length === 0) {
      setSelectedProviderId('new')
      return
    }

    const hasSelectedProvider = providerAccounts.some(
      (provider) => provider.id === selectedProviderId,
    )
    if (selectedProviderId !== 'new' && !hasSelectedProvider) {
      setSelectedProviderId(providerAccounts[0].id)
    }
  }, [providerAccounts, selectedProviderId, setSelectedProviderId])

  const selectedProvider = useMemo<ProviderAccount | null>(() => {
    if (selectedProviderId === 'new') {
      return null
    }
    return providerAccounts.find((provider) => provider.id === selectedProviderId) ?? null
  }, [providerAccounts, selectedProviderId])

  function handleProviderSelection(nextProviderId: string | 'new') {
    setSelectedProviderId(nextProviderId)
  }

  async function handleSaveProvider(value: {
    profile_name: string
    base_url: string
    api_key: string
    initial_model?: string
  }) {
    setAccountBusy(true)
    try {
      if (selectedProvider) {
        const updated = await getAppClient().request<ProviderAccount>('providers.update', {
          id: selectedProvider.id,
          ...value,
        })
        handleProviderSelection(updated.id)
        toastSuccess('服务商配置已更新。')
        return
      }
      const created = await getAppClient().request<ProviderAccount>('providers.create', {
        profile_name: value.profile_name,
        base_url: value.base_url,
        api_key: value.api_key,
      })
      handleProviderSelection(created.id)
      const initialModelId = value.initial_model?.trim()
      if (initialModelId) {
        await getAppClient().request<ProviderModel>('providers.models.create', {
          provider_id: created.id,
          model_name: initialModelId,
          model: initialModelId,
        })
        toastSuccess('服务商与首个模型已创建。')
      } else {
        toastSuccess('服务商已创建。')
      }
    } catch (error) {
      toastError(errorMessageFromUnknown(error))
    } finally {
      setAccountBusy(false)
    }
  }

  async function handleCreateModel(value: { model: string }) {
    if (!selectedProvider) return
    setModelBusyId('create')
    try {
      await getAppClient().request<ProviderModel>('providers.models.create', {
        provider_id: selectedProvider.id,
        model_name: value.model,
        model: value.model,
      })
      toastSuccess('模型已添加。')
    } catch (error) {
      toastError(errorMessageFromUnknown(error))
    } finally {
      setModelBusyId(null)
    }
  }

  async function handleUpdateModel(modelId: string, value: { model: string }) {
    setModelBusyId(`save:${modelId}`)
    try {
      await getAppClient().request<ProviderModel>('providers.models.update', {
        id: modelId,
        model_name: value.model,
        model: value.model,
      })
      toastSuccess('模型配置已更新。')
    } catch (error) {
      toastError(errorMessageFromUnknown(error))
    } finally {
      setModelBusyId(null)
    }
  }

  async function handleDeleteModel(modelId: string) {
    if (selectedProvider && selectedProvider.models.length <= 1) {
      toastError('至少保留一个模型。请先添加新模型，再删除当前模型。')
      return
    }
    setModelBusyId(`delete:${modelId}`)
    try {
      await getAppClient().request('providers.models.delete', {
        id: modelId,
      })
      toastSuccess('模型已移除。')
    } catch (error) {
      toastError(errorMessageFromUnknown(error))
    } finally {
      setModelBusyId(null)
    }
  }

  async function handleTestModel(value: { provider_id: string; model: string; model_id?: string }) {
    const busyKey = value.model_id ? `test:${value.model_id}` : `test:${value.model}`
    setModelBusyId(busyKey)
    try {
      await getAppClient().request('providers.models.test', value)
      toastSuccess('模型连接测试成功。')
    } catch (error) {
      toastError(errorMessageFromUnknown(error))
    } finally {
      setModelBusyId(null)
    }
  }

  return (
    <ProviderSettingsSection
      accountBusy={accountBusy}
      modelBusyId={modelBusyId}
      onCreateModel={handleCreateModel}
      onDeleteModel={handleDeleteModel}
      onNewProvider={() => handleProviderSelection('new')}
      onSaveProvider={handleSaveProvider}
      onTestModel={handleTestModel}
      onUpdateModel={handleUpdateModel}
      providers={providerAccounts}
      selectedProvider={selectedProvider}
      selectedProviderId={selectedProviderId}
      setSelectedProviderId={handleProviderSelection}
    />
  )
}
