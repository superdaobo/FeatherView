/**
 * 设置 store：读取、持久化、主题应用。
 */
import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import type { ReaderSettings } from '../types'
import { DEFAULT_SETTINGS, loadSettings, resolveTheme, saveSettings } from '../services/settingsService'

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<ReaderSettings>(loadSettings())

  watch(
    settings,
    (value) => {
      saveSettings(value)
      applyTheme()
    },
    { deep: true },
  )

  function applyTheme(): void {
    const theme = resolveTheme(settings.value.theme)
    document.documentElement.dataset.theme = theme
  }

  function setTheme(theme: ReaderSettings['theme']): void {
    settings.value.theme = theme
  }

  function adjustFontSize(delta: number): void {
    const next = Math.min(32, Math.max(11, settings.value.fontSize + delta))
    settings.value.fontSize = next
  }

  function resetFontSize(): void {
    settings.value.fontSize = DEFAULT_SETTINGS.fontSize
  }

  function toggleTheme(): void {
    const map: Record<ReaderSettings['theme'], ReaderSettings['theme']> = {
      system: 'light',
      light: 'dark',
      dark: 'system',
    }
    settings.value.theme = map[settings.value.theme]
  }

  // 系统主题变化时跟随
  if (typeof window !== 'undefined' && window.matchMedia) {
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener?.('change', () => {
      if (settings.value.theme === 'system') applyTheme()
    })
  }

  return {
    settings,
    applyTheme,
    setTheme,
    adjustFontSize,
    resetFontSize,
    toggleTheme,
  }
})
