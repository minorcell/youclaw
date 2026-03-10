import {
  createContext,
  useContext,
  useMemo,
  type PropsWithChildren,
} from "react"
import { toast, type ExternalToast } from "sonner"

import { Toaster } from "@/components/ui/sonner"

type ToastId = string | number

interface ToastContextValue {
  show: (message: string, options?: ExternalToast) => ToastId
  success: (message: string, options?: ExternalToast) => ToastId
  info: (message: string, options?: ExternalToast) => ToastId
  warning: (message: string, options?: ExternalToast) => ToastId
  error: (message: string, options?: ExternalToast) => ToastId
  loading: (message: string, options?: ExternalToast) => ToastId
  dismiss: (id?: ToastId) => void
}

const ToastContext = createContext<ToastContextValue | null>(null)

export function ToastProvider({ children }: PropsWithChildren) {
  const value = useMemo<ToastContextValue>(
    () => ({
      show: (message, options) => toast(message, options),
      success: (message, options) => toast.success(message, options),
      info: (message, options) => toast.info(message, options),
      warning: (message, options) => toast.warning(message, options),
      error: (message, options) => toast.error(message, options),
      loading: (message, options) => toast.loading(message, options),
      dismiss: (id) => {
        if (id === undefined) {
          toast.dismiss()
          return
        }
        toast.dismiss(id)
      },
    }),
    [],
  )

  return (
    <ToastContext.Provider value={value}>
      {children}
      <Toaster position="top-center" richColors />
    </ToastContext.Provider>
  )
}

export function useToastContext() {
  const context = useContext(ToastContext)
  if (!context) {
    throw new Error("useToastContext must be used within ToastProvider")
  }
  return context
}
