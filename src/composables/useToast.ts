/**
 * 轻量 Toast 通知（全局单例，不依赖任何 UI 框架）。
 */
import { reactive } from 'vue'

export interface ToastItem {
  id: number
  title: string
  message?: string
  kind: 'info' | 'error' | 'success'
}

const state = reactive<{ toasts: ToastItem[] }>({ toasts: [] })
let nextId = 1

export function useToast() {
  function show(title: string, opts: { message?: string; kind?: ToastItem['kind']; duration?: number } = {}) {
    const id = nextId++
    const item: ToastItem = {
      id,
      title,
      message: opts.message,
      kind: opts.kind ?? 'info',
    }
    state.toasts.push(item)
    const duration = opts.duration ?? 4000
    setTimeout(() => dismiss(id), duration)
  }

  function dismiss(id: number): void {
    const idx = state.toasts.findIndex((t) => t.id === id)
    if (idx >= 0) state.toasts.splice(idx, 1)
  }

  return { state, show, dismiss }
}
