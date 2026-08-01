import { describe, expect, it } from 'vitest'
import {
  DEFAULT_SETTINGS,
  loadSettings,
  resolveTheme,
  sanitizeSettings,
  saveSettings,
} from '../settingsService'

describe('settingsService', () => {
  it('has sensible defaults for chinese reading', () => {
    expect(DEFAULT_SETTINGS.fontSize).toBeGreaterThanOrEqual(14)
    expect(DEFAULT_SETTINGS.lineHeight).toBeGreaterThanOrEqual(1.5)
    expect(DEFAULT_SETTINGS.theme).toBe('system')
    expect(DEFAULT_SETTINGS.wordWrap).toBe(true)
  })

  it('returns defaults when storage is empty', () => {
    expect(loadSettings()).toEqual(DEFAULT_SETTINGS)
  })

  it('persists and restores settings', () => {
    saveSettings({ ...DEFAULT_SETTINGS, fontSize: 20, theme: 'dark' })
    const loaded = loadSettings()
    expect(loaded.fontSize).toBe(20)
    expect(loaded.theme).toBe('dark')
  })

  it('sanitizes invalid values', () => {
    const cleaned = sanitizeSettings({
      ...DEFAULT_SETTINGS,
      fontSize: 999,
      lineHeight: 0.1,
      theme: 'blue' as never,
      contentWidth: 'huge' as never,
      fontFamily: 'comic' as never,
    })
    expect(cleaned.fontSize).toBe(32)
    expect(cleaned.lineHeight).toBe(1.2)
    expect(cleaned.theme).toBe('system')
    expect(cleaned.contentWidth).toBe('medium')
    expect(cleaned.fontFamily).toBe('system')
  })

  it('resolves system theme from media query', () => {
    expect(['light', 'dark']).toContain(resolveTheme('system'))
    expect(resolveTheme('light')).toBe('light')
    expect(resolveTheme('dark')).toBe('dark')
  })
})
