import { ref } from 'vue'

export type ToastType = 'ok' | 'error' | 'info' | 'warn'

export type Toast = {
  id: number
  type: ToastType
  message: string
  timer?: ReturnType<typeof setTimeout>
}

// 모듈 수준 싱글톤 (전역 공유)
const toasts = ref<Toast[]>([])
let counter = 0

export function showToast(message: string, type: ToastType = 'info') {
  const id = ++counter
  const toast: Toast = { id, type, message }

  if (type !== 'error') {
    toast.timer = setTimeout(() => dismissToast(id), 3000)
  }
  toasts.value.push(toast)
}

export function dismissToast(id: number) {
  const idx = toasts.value.findIndex((t) => t.id === id)
  if (idx === -1) return
  const t = toasts.value[idx]
  if (t.timer) clearTimeout(t.timer)
  toasts.value.splice(idx, 1)
}

export function useToastList() {
  return { toasts, dismissToast }
}
