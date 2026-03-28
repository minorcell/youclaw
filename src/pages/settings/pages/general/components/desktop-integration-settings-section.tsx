import { invoke, isTauri } from '@tauri-apps/api/core'
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from '@tauri-apps/plugin-autostart'
import { isPermissionGranted, requestPermission } from '@tauri-apps/plugin-notification'
import { Bell, LaptopMinimalCheck, Loader2, PanelTop } from 'lucide-react'
import { useEffect, useState, type ReactNode } from 'react'

import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Switch } from '@/components/ui/switch'
import { useToastContext } from '@/contexts/toast-context'
import {
  SETTINGS_CARD_CLASSNAME,
  SETTINGS_CARD_CONTENT_CLASSNAME,
  SETTINGS_CARD_HEADER_CLASSNAME,
} from '@/pages/settings/lib/ui'

function errorText(error: unknown): string {
  if (error instanceof Error) return error.message
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

function permissionLabel(permission: NotificationPermission): string {
  if (permission === 'granted') return '已允许'
  if (permission === 'denied') return '已拒绝'
  return '未授权'
}

function permissionDescription(permission: NotificationPermission): string {
  if (permission === 'granted') {
    return '系统通知已就绪，可用于接收长任务或重要状态提醒。'
  }
  if (permission === 'denied') {
    return '当前已被系统拒绝，需要到系统设置中手动重新开启。'
  }
  return '首次使用时需要系统授权，关闭权限也需要在系统设置中调整。'
}

export function DesktopIntegrationSettingsSection() {
  const { success: toastSuccess, error: toastError, info: toastInfo } = useToastContext()
  const desktopSupported = isTauri()
  const [loading, setLoading] = useState(desktopSupported)
  const [menuBarEnabled, setMenuBarEnabled] = useState(false)
  const [autostartEnabled, setAutostartEnabled] = useState(false)
  const [notificationPermission, setNotificationPermission] =
    useState<NotificationPermission>('default')
  const [savingMenuBar, setSavingMenuBar] = useState(false)
  const [savingAutostart, setSavingAutostart] = useState(false)
  const [requestingNotificationPermission, setRequestingNotificationPermission] = useState(false)
  const [sendingTestNotification, setSendingTestNotification] = useState(false)

  useEffect(() => {
    if (!desktopSupported) {
      setLoading(false)
      return
    }

    let cancelled = false
    setLoading(true)

    void Promise.all([
      invoke<boolean>('get_menu_bar_enabled'),
      isAutostartEnabled(),
      isPermissionGranted(),
    ])
      .then(([nextMenuBarEnabled, nextAutostartEnabled, nextNotificationGranted]) => {
        if (cancelled) return
        setMenuBarEnabled(nextMenuBarEnabled)
        setAutostartEnabled(nextAutostartEnabled)
        setNotificationPermission(nextNotificationGranted ? 'granted' : 'default')
      })
      .catch((error) => {
        if (!cancelled) {
          toastError(errorText(error))
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false)
        }
      })

    return () => {
      cancelled = true
    }
  }, [desktopSupported, toastError])

  async function handleMenuBarChange(nextEnabled: boolean) {
    if (!desktopSupported) return
    setSavingMenuBar(true)
    try {
      const confirmed = await invoke<boolean>('set_menu_bar_enabled', {
        enabled: nextEnabled,
      })
      setMenuBarEnabled(confirmed)
      toastSuccess(confirmed ? '菜单栏图标已开启。' : '菜单栏图标已关闭。')
    } catch (error) {
      toastError(errorText(error))
    } finally {
      setSavingMenuBar(false)
    }
  }

  async function handleAutostartChange(nextEnabled: boolean) {
    if (!desktopSupported) return
    setSavingAutostart(true)
    try {
      if (nextEnabled) {
        await enableAutostart()
      } else {
        await disableAutostart()
      }
      const confirmed = await isAutostartEnabled()
      setAutostartEnabled(confirmed)
      toastSuccess(confirmed ? '已开启开机启动。' : '已关闭开机启动。')
    } catch (error) {
      toastError(errorText(error))
    } finally {
      setSavingAutostart(false)
    }
  }

  async function handleRequestNotificationPermission() {
    if (!desktopSupported) return
    setRequestingNotificationPermission(true)
    try {
      const permission = await requestPermission()
      setNotificationPermission(permission)
      if (permission === 'granted') {
        toastSuccess('通知权限已开启。')
        return
      }
      if (permission === 'denied') {
        toastInfo('通知权限被系统拒绝，请到系统设置中重新开启。')
        return
      }
      toastInfo('通知权限尚未授权。')
    } catch (error) {
      toastError(errorText(error))
    } finally {
      setRequestingNotificationPermission(false)
    }
  }

  async function handleSendTestNotification() {
    if (!desktopSupported) return
    setSendingTestNotification(true)
    try {
      await invoke('send_test_notification')
      toastSuccess(
        import.meta.env.DEV
          ? '已提交测试通知。若没看到横幅，请检查通知中心；macOS 当前可能因专注模式/勿扰而延后展示。'
          : '已提交测试通知，请留意系统通知中心。',
      )
    } catch (error) {
      toastError(errorText(error))
    } finally {
      setSendingTestNotification(false)
    }
  }

  if (loading) {
    return (
      <Card className={SETTINGS_CARD_CLASSNAME}>
        <CardHeader className={SETTINGS_CARD_HEADER_CLASSNAME}>
          <CardTitle>桌面集成</CardTitle>
          <CardDescription>正在加载菜单栏、开机启动和通知权限状态。</CardDescription>
        </CardHeader>
        <CardContent className={SETTINGS_CARD_CONTENT_CLASSNAME}>
          <div className='rounded-xl bg-background/85 px-4 py-5 text-sm text-muted-foreground'>
            <Loader2 className='mr-2 inline h-4 w-4 animate-spin' />
            正在读取桌面能力状态...
          </div>
        </CardContent>
      </Card>
    )
  }

  return (
    <Card className={SETTINGS_CARD_CLASSNAME}>
      <CardHeader className={SETTINGS_CARD_HEADER_CLASSNAME}>
        <CardTitle>桌面集成</CardTitle>
        <CardDescription>配置菜单栏入口、开机启动与系统通知权限。</CardDescription>
      </CardHeader>
      <CardContent className={SETTINGS_CARD_CONTENT_CLASSNAME}>
        {!desktopSupported ? (
          <div className='rounded-xl bg-background/85 px-4 py-5 text-sm text-muted-foreground'>
            仅在 Tauri 桌面环境中提供这些系统级交互。
          </div>
        ) : (
          <div className='space-y-3'>
            <DesktopSettingRow
              control={
                <div className='flex items-center gap-2'>
                  {savingMenuBar ? <Loader2 className='h-4 w-4 animate-spin' /> : null}
                  <Switch
                    checked={menuBarEnabled}
                    disabled={savingMenuBar}
                    onCheckedChange={handleMenuBarChange}
                  />
                </div>
              }
              description='在 macOS 菜单栏或系统托盘中保留 YouClaw 的快速入口。'
              icon={<PanelTop className='h-4 w-4 text-muted-foreground' />}
              status={menuBarEnabled ? '已启用' : '已关闭'}
              title='菜单栏图标'
            />

            <DesktopSettingRow
              control={
                <div className='flex items-center gap-2'>
                  {savingAutostart ? <Loader2 className='h-4 w-4 animate-spin' /> : null}
                  <Switch
                    checked={autostartEnabled}
                    disabled={savingAutostart}
                    onCheckedChange={handleAutostartChange}
                  />
                </div>
              }
              description='登录系统后自动启动 YouClaw，适合常驻在后台待命。'
              icon={<LaptopMinimalCheck className='h-4 w-4 text-muted-foreground' />}
              status={autostartEnabled ? '已开启' : '未开启'}
              title='开机启动'
            />

            <DesktopSettingRow
              control={
                notificationPermission === 'granted' ? (
                  <Button
                    disabled={sendingTestNotification}
                    onClick={() => void handleSendTestNotification()}
                    size='sm'
                    type='button'
                    variant='secondary'
                  >
                    {sendingTestNotification ? (
                      <Loader2 className='mr-1 h-3.5 w-3.5 animate-spin' />
                    ) : null}
                    发送测试通知
                  </Button>
                ) : (
                  <Button
                    disabled={
                      requestingNotificationPermission || notificationPermission === 'denied'
                    }
                    onClick={() => void handleRequestNotificationPermission()}
                    size='sm'
                    type='button'
                    variant='secondary'
                  >
                    {requestingNotificationPermission ? (
                      <Loader2 className='mr-1 h-3.5 w-3.5 animate-spin' />
                    ) : null}
                    {notificationPermission === 'denied' ? '请前往系统设置开启' : '请求权限'}
                  </Button>
                )
              }
              description={permissionDescription(notificationPermission)}
              icon={<Bell className='h-4 w-4 text-muted-foreground' />}
              status={permissionLabel(notificationPermission)}
              title='通知权限'
            />
          </div>
        )}
      </CardContent>
    </Card>
  )
}

function DesktopSettingRow({
  icon,
  title,
  description,
  status,
  control,
}: {
  icon: ReactNode
  title: string
  description: string
  status: string
  control: ReactNode
}) {
  return (
    <div className='flex flex-col gap-3 rounded-xl bg-background/85 px-3 py-3 sm:flex-row sm:items-center sm:justify-between'>
      <div className='flex min-w-0 items-start gap-3'>
        <span className='mt-0.5 inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-card'>
          {icon}
        </span>
        <div className='min-w-0'>
          <div className='flex flex-wrap items-center gap-2'>
            <p className='text-sm font-medium'>{title}</p>
            <span className='rounded-full bg-card px-2 py-0.5 text-[11px] text-muted-foreground'>
              {status}
            </span>
          </div>
          <p className='mt-1 text-xs text-muted-foreground'>{description}</p>
        </div>
      </div>
      <div className='flex shrink-0 items-center justify-end'>{control}</div>
    </div>
  )
}
