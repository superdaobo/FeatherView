/**
 * 设置服务：ReaderSettings 默认值与 localStorage 持久化。
 */
import type { ReaderSettings } from '../types'

const STORAGE_KEY = 'featherview.settings'

/** 默认值：适合普通中文阅读 */
export const DEFAULT_SETTINGS: ReaderSettings = {
  theme: 'system',
  fontSize: 16,
  lineHeight: 1.8,
  contentWidth: 'medium',
  wordWrap: true,
  showLineNumbers: false,
  fontFamily: 'system',
  rememberReadingPosition: true,
}

export function loadSettings(): ReaderSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return { ...DEFAULT_SETTINGS }
    const parsed = JSON.parse(raw)
    return sanitizeSettings({ ...DEFAULT_SETTINGS, ...parsed })
  } catch {
    return { ...DEFAULT_SETTINGS }
  }
}

export function saveSettings(settings: ReaderSettings): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
  } catch {
    // 静默
  }
}

/** 校验并修正非法值 */
export function sanitizeSettings(s: ReaderSettings): ReaderSettings {
  const settings = { ...s }
  if (!['system', 'light', 'dark'].includes(settings.theme)) settings.theme = 'system'
  if (!Number.isFinite(settings.fontSize)) settings.fontSize = DEFAULT_SETTINGS.fontSize
  settings.fontSize = Math.min(32, Math.max(11, Math.round(settings.fontSize)))
  if (!Number.isFinite(settings.lineHeight)) settings.lineHeight = DEFAULT_SETTINGS.lineHeight
  settings.lineHeight = Math.min(2.6, Math.max(1.2, settings.lineHeight))
  if (!['narrow', 'medium', 'wide', 'full'].includes(settings.contentWidth)) {
    settings.contentWidth = 'medium'
  }
  settings.wordWrap = Boolean(settings.wordWrap)
  settings.showLineNumbers = Boolean(settings.showLineNumbers)
  if (!['system', 'serif', 'monospace'].includes(settings.fontFamily)) settings.fontFamily = 'system'
  settings.rememberReadingPosition = settings.rememberReadingPosition !== false
  return settings
}

/** 根据主题设置解析出实际生效的主题 */
export function resolveTheme(theme: ReaderSettings['theme']): 'light' | 'dark' {
  if (theme === 'system') {
    return window.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
  }
  return theme
}
