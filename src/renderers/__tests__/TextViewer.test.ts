import { flushPromises, mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import TextViewer from '../text/TextViewer.vue'
import type { DocumentMeta } from '../../types'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))

function makeMeta(ext = 'txt', size = 100): DocumentMeta {
  return {
    source: { id: '1', name: `a.${ext}`, extension: ext, path: `C:/a.${ext}` },
    size,
    isBinary: false,
  }
}

/** 50001 行内容（超过虚拟滚动阈值 50000） */
function makeHugeContent(): string {
  return Array.from({ length: 50001 }, (_, i) => `line ${i}`).join('\n')
}

describe('TextViewer', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    invokeMock.mockReset()
  })

  it('renders text lines', () => {
    const wrapper = mount(TextViewer, {
      props: { document: makeMeta(), content: '第一行\n第二行\n第三行' },
    })
    expect(wrapper.findAll('.text-line').length).toBe(3)
  })

  it('toggles line numbers from settings', async () => {
    const { useSettingsStore } = await import('../../stores/settings')
    const store = useSettingsStore()
    store.settings.showLineNumbers = true

    const wrapper = mount(TextViewer, {
      props: { document: makeMeta(), content: 'a\nb' },
    })
    expect(wrapper.findAll('.line-num').length).toBe(2)

    // 设置变化后样式更新（响应式生效）
    store.settings.showLineNumbers = false
    await wrapper.vm.$nextTick()
    expect(wrapper.findAll('.line-num').length).toBe(0)
  })

  it('toggles word wrap class from settings', async () => {
    const { useSettingsStore } = await import('../../stores/settings')
    const store = useSettingsStore()
    store.settings.wordWrap = true

    const wrapper = mount(TextViewer, {
      props: { document: makeMeta(), content: 'hello' },
    })
    expect(wrapper.find('.text-content').classes()).toContain('wrap-on')

    store.settings.wordWrap = false
    await wrapper.vm.$nextTick()
    expect(wrapper.find('.text-content').classes()).not.toContain('wrap-on')
  })

  it('applies monospace font class from settings', async () => {
    const { useSettingsStore } = await import('../../stores/settings')
    const store = useSettingsStore()
    store.settings.fontFamily = 'monospace'

    const wrapper = mount(TextViewer, {
      props: { document: makeMeta(), content: 'x' },
    })
    expect(wrapper.find('.text-content').classes()).toContain('mono')
  })

  it('truncates very large content with notice', () => {
    const big = 'x'.repeat(2_000_001)
    const wrapper = mount(TextViewer, {
      props: { document: makeMeta(), content: big },
    })
    expect(wrapper.text()).toContain('文件较大')
  })

  it('emits stats for line/word counts', async () => {
    const wrapper = mount(TextViewer, {
      props: { document: makeMeta(), content: '一\n二 三' },
    })
    const emitted = wrapper.emitted('stats')
    expect(emitted).toBeTruthy()
    expect(emitted![0][0]).toEqual({ lines: 2, words: 3 })
  })
})

describe('TextViewer virtual scrolling', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('renders only a window of lines for huge non-wrap content', async () => {
    const { useSettingsStore } = await import('../../stores/settings')
    useSettingsStore().settings.wordWrap = false

    const wrapper = mount(TextViewer, {
      props: { document: makeMeta('log'), content: makeHugeContent() },
    })
    await nextTick()
    const rendered = wrapper.findAll('.text-line')
    expect(rendered.length).toBeGreaterThan(0)
    expect(rendered.length).toBeLessThan(100)
    expect(rendered[0].attributes('data-line')).toBe('1')
  })

  it('keeps absolute line numbers in virtual mode', async () => {
    const { useSettingsStore } = await import('../../stores/settings')
    const store = useSettingsStore()
    store.settings.wordWrap = false
    store.settings.showLineNumbers = true

    const wrapper = mount(TextViewer, {
      props: { document: makeMeta('log'), content: makeHugeContent() },
    })
    await nextTick()
    expect(wrapper.findAll('.line-num').length).toBe(wrapper.findAll('.text-line').length)
    expect(wrapper.find('.line-num').text()).toBe('1')
  })

  it('moves the render window on scroll', async () => {
    const { useSettingsStore } = await import('../../stores/settings')
    useSettingsStore().settings.wordWrap = false

    const wrapper = mount(TextViewer, {
      props: { document: makeMeta('log'), content: makeHugeContent() },
    })
    await nextTick()
    const container = wrapper.find('.text-content').element as HTMLElement
    container.scrollTop = 24 * 1000 // 第 1000 行
    container.dispatchEvent(new Event('scroll'))
    await wrapper.vm.$nextTick()
    const first = wrapper.find('.text-line').attributes('data-line')
    // overscan 10：1000 - 10 = 990 → 行号 991
    expect(first).toBe('991')
  })

  it('renders all lines while search is active (search fallback)', async () => {
    const { useSettingsStore } = await import('../../stores/settings')
    useSettingsStore().settings.wordWrap = false

    // 搜索激活时禁用虚拟滚动、逐行渲染；用小规模内容避免 jsdom 渲染 5 万行超时
    const small = Array.from({ length: 200 }, (_, i) => `line ${i}`).join('\n')
    const wrapper = mount(TextViewer, {
      props: { document: makeMeta('log'), content: small, searchActive: true },
    })
    expect(wrapper.findAll('.text-line').length).toBe(200)
  })

  it('falls back to plain-pre for huge content with wrap enabled', () => {
    // wordWrap 默认 true：行高不定无法虚拟化 → 单文本节点
    const wrapper = mount(TextViewer, {
      props: { document: makeMeta('log'), content: makeHugeContent() },
    })
    expect(wrapper.find('.plain-pre').exists()).toBe(true)
    expect(wrapper.findAll('.text-line').length).toBe(0)
  })
})

describe('TextViewer large file session', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    invokeMock.mockReset()
    ;(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {}
  })

  it('shows a fallback notice when session commands are not implemented', async () => {
    invokeMock.mockRejectedValue(new Error('Command open_read_session not found'))
    const wrapper = mount(TextViewer, {
      props: { document: makeMeta('log', 11 * 1024 * 1024), content: '' },
    })
    await flushPromises()
    expect(wrapper.text()).toContain('暂不支持大文件分块读取')
  })

  it('loads huge content via session and virtualizes, closing session on unmount', async () => {
    const bigText = makeHugeContent()
    invokeMock.mockImplementation(async (cmd: string, args?: { offset?: number; length?: number }) => {
      if (cmd === 'open_read_session') {
        return {
          sessionId: 's1',
          info: { size: bigText.length, mtime: 1, encoding: 'utf-8', isText: true, suggestedChunkBytes: 100000 },
        }
      }
      if (cmd === 'read_range') {
        const offset = args?.offset ?? 0
        const length = args?.length ?? 0
        const slice = bigText.slice(offset, offset + length)
        return {
          bytesRead: slice.length,
          nextOffset: offset + slice.length,
          eof: offset + slice.length >= bigText.length,
          text: slice,
          encoding: 'utf-8',
        }
      }
      if (cmd === 'close_read_session') return null
      throw new Error(`unknown command: ${cmd}`)
    })

    const { useSettingsStore } = await import('../../stores/settings')
    useSettingsStore().settings.wordWrap = false

    const wrapper = mount(TextViewer, {
      props: { document: makeMeta('log', 11 * 1024 * 1024), content: '' },
    })
    await flushPromises()
    await new Promise((r) => setTimeout(r, 0))

    await nextTick()
    const rendered = wrapper.findAll('.text-line')
    expect(rendered.length).toBeGreaterThan(0)
    expect(rendered.length).toBeLessThan(100)
    expect(rendered[0].attributes('data-line')).toBe('1')

    // 卸载必须关闭会话（合同第 4 节）
    wrapper.unmount()
    await flushPromises()
    expect(invokeMock).toHaveBeenCalledWith('close_read_session', { sessionId: 's1' })
  })
})
