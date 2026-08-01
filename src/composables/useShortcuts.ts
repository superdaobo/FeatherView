/**
 * 全局快捷键管理。
 *
 * Ctrl+O 打开文件 / Ctrl+F 文档搜索 / Ctrl+Plus 放大 / Ctrl+Minus 缩小 /
 * Ctrl+0 恢复字号 / Ctrl+Shift+T 切换主题 / Esc 关闭
 *
 * 输入框（input/textarea/contenteditable）聚焦时除 Esc 外不触发，
 * 避免影响正常输入。
 */
import { onBeforeUnmount, onMounted } from 'vue'

export interface ShortcutHandlers {
  onOpenFile?: () => void
  onSearch?: () => void
  onFontIncrease?: () => void
  onFontDecrease?: () => void
  onFontReset?: () => void
  onToggleTheme?: () => void
  onEscape?: () => void
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  const tag = target.tagName
  return tag === 'INPUT' || tag === 'TEXTAREA' || target.isContentEditable
}

export function useShortcuts(handlers: ShortcutHandlers): void {
  function onKeyDown(event: KeyboardEvent): void {
    const editable = isEditableTarget(event.target)

    // Esc 永远可用
    if (event.key === 'Escape') {
      handlers.onEscape?.()
      return
    }

    if (editable) return

    const ctrl = event.ctrlKey || event.metaKey
    if (!ctrl) return

    const key = event.key.toLowerCase()

    switch (key) {
      case 'o':
        event.preventDefault()
        handlers.onOpenFile?.()
        break
      case 'f':
        event.preventDefault()
        handlers.onSearch?.()
        break
      case '+':
      case '=':
        event.preventDefault()
        handlers.onFontIncrease?.()
        break
      case '-':
      case '_':
        event.preventDefault()
        handlers.onFontDecrease?.()
        break
      case '0':
        event.preventDefault()
        handlers.onFontReset?.()
        break
      case 't':
        if (event.shiftKey) {
          event.preventDefault()
          handlers.onToggleTheme?.()
        }
        break
    }
  }

  onMounted(() => window.addEventListener('keydown', onKeyDown))
  onBeforeUnmount(() => window.removeEventListener('keydown', onKeyDown))
}
