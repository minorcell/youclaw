import { Toaster as Sonner, type ToasterProps } from 'sonner'
import {
  CircleCheckIcon,
  InfoIcon,
  TriangleAlertIcon,
  OctagonXIcon,
  Loader2Icon,
} from 'lucide-react'
import { useSettingsStore } from '@/store/settings-store'

function isDarkHexColor(hex: string): boolean {
  const normalized = hex.trim().toLowerCase()
  if (!/^#[0-9a-f]{6}$/.test(normalized)) {
    return false
  }
  const red = Number.parseInt(normalized.slice(1, 3), 16)
  const green = Number.parseInt(normalized.slice(3, 5), 16)
  const blue = Number.parseInt(normalized.slice(5, 7), 16)
  const brightness = (red * 299 + green * 587 + blue * 114) / 1000
  return brightness <= 145
}

const Toaster = ({ ...props }: ToasterProps) => {
  const mode = useSettingsStore((state) => state.mode)
  const customBackground = useSettingsStore((state) => state.custom.background)

  let theme: ToasterProps['theme'] = 'light'
  if (mode === 'black') {
    theme = 'dark'
  } else if (mode === 'custom') {
    theme = isDarkHexColor(customBackground) ? 'dark' : 'light'
  }

  return (
    <Sonner
      theme={theme}
      className='toaster group'
      icons={{
        success: <CircleCheckIcon className='size-4' />,
        info: <InfoIcon className='size-4' />,
        warning: <TriangleAlertIcon className='size-4' />,
        error: <OctagonXIcon className='size-4' />,
        loading: <Loader2Icon className='size-4 animate-spin' />,
      }}
      style={
        {
          '--normal-bg': 'var(--popover)',
          '--normal-text': 'var(--popover-foreground)',
          '--normal-border': 'var(--border)',
          '--border-radius': 'var(--radius)',
        } as React.CSSProperties
      }
      toastOptions={{
        classNames: {
          toast: 'cn-toast',
        },
      }}
      {...props}
    />
  )
}

export { Toaster }
