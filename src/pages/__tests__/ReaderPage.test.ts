import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import ReaderPage from '../ReaderPage.vue'
import { useDocumentStore } from '../../stores/document'
import { useSettingsStore } from '../../stores/settings'
import { setupRenderers } from '../../renderers'
import { useToast } from '../../composables/useToast'
import { getDocumentIdentity } from '../../services/platformService'
import { clearPositions, savePosition } from '../../services/readingPositionService'
import { DEFAULT_WINDOW_TITLE } from '../../services/windowService'
import type { DocumentMeta, ReadingPositionRecord } from '../../types'

vi.mock('vue-router', () => ({
  useRouter: () => ({
    push: vi.fn(),
    back: vi.fn(),
    currentRoute: { value: { name: 'reader' } },
  }),
}))

const MD_PATH = 'C:/docs/readme.md'
const MOCK_HEIGHT = 2000
const MOCK_VIEWPORT = 800

// jsdom 的 scrollHeight/clientHeight 恒为 0，统一 mock 以模拟真实渲染完成的高度
beforeAll(() => {
  Object.defineProperty(Element.prototype, 'scrollHeight', {
    configurable: true,
    get() {
      return MOCK_HEIGHT
    },
  })
  Object.defineProperty(Element.prototype, 'clientHeight', {
    configurable: true,
    get() {
      return MOCK_VIEWPORT
    },
  })
})

function makeMeta(): DocumentMeta {
  return {
    source: { id: 't1', name: 'readme.md', extension: 'md', path: MD_PATH },
    size: 100,
    isBinary: false,
  }
}

function makeRecord(overrides: Partial<ReadingPositionRecord> = {}): ReadingPositionRecord {
  return {
    sourceId: getDocumentIdentity(makeMeta().source),
    path: MD_PATH,
    rendererId: 'markdown',
    scrollTop: 400,
    scrollRatio: 0.5,
    viewportHeight: MOCK_VIEWPORT,
    contentHeight: MOCK_HEIGHT,
    updatedAt: Date.now(),
    ...overrides,
  }
}

function setupStoreReady(): ReturnType<typeof useDocumentStore> {
  const store = useDocumentStore()
  store.status = 'ready'
  store.source = makeMeta().source
  store.meta = makeMeta()
  store.result = { content: '# 标题\n\n正文内容\n\n## 第二章\n\n更多内容', isBinary: false, size: 100 }
  return store
}

/** 等渲染器异步加载 + rAF 恢复循环 */
async function settle(): Promise<void> {
  await flushPromises()
  await new Promise((r) => setTimeout(r, 50))
  await new Promise((r) => requestAnimationFrame(() => r(null)))
  await new Promise((r) => requestAnimationFrame(() => r(null)))
}

async function getMarkdownBody(wrapper: ReturnType<typeof mount>): Promise<HTMLElement | undefined> {
  // 渲染器为异步组件，轮询等待其挂载完成
  for (let i = 0; i < 40; i++) {
    const el = wrapper.find('.markdown-body')
    if (el.exists()) return el.element as HTMLElement
    await new Promise((r) => setTimeout(r, 25))
  }
  return undefined
}

describe('ReaderPage interactions', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
    clearPositions()
    setupRenderers()
    document.title = DEFAULT_WINDOW_TITLE
  })

  it('restores scroll position after render when record exists', async () => {
    setupStoreReady()
    savePosition(makeRecord())

    const wrapper = mount(ReaderPage)
    await settle()

    const container = await getMarkdownBody(wrapper)
    expect(container).toBeDefined()
    // 恢复在 watch+rAF 链中异步执行，轮询等待
    let actual = container!.scrollTop
    for (let i = 0; i < 20 && actual !== 400; i++) {
      await new Promise((r) => setTimeout(r, 25))
      actual = container!.scrollTop
    }
    // 内容高度与历史一致 → 使用绝对位置
    expect(actual).toBe(400)
    // Toast 渲染在 App.vue 容器，通过全局 Toast 状态验证（且只出现一次）
    const { state: toastState } = useToast()
    const restoreToasts = toastState.toasts.filter((t) => t.title === '已恢复上次阅读位置')
    expect(restoreToasts.length).toBe(1)
    wrapper.unmount()
  })

  it('starts from top on first open (no record)', async () => {
    setupStoreReady()
    const wrapper = mount(ReaderPage)
    await settle()

    expect((await getMarkdownBody(wrapper))?.scrollTop ?? 0).toBe(0)
    expect(wrapper.text()).not.toContain('已恢复上次阅读位置')
    wrapper.unmount()
  })

  it('does not restore when rememberReadingPosition is disabled', async () => {
    setupStoreReady()
    savePosition(makeRecord())
    const settings = useSettingsStore()
    settings.settings.rememberReadingPosition = false

    const wrapper = mount(ReaderPage)
    await settle()

    expect((await getMarkdownBody(wrapper))?.scrollTop ?? 0).toBe(0)
    wrapper.unmount()
  })

  it('restores by ratio when content height changed', async () => {
    setupStoreReady()
    // 历史内容高度与当前不同（变化超过容差）→ 按比例恢复
    savePosition(makeRecord({ contentHeight: 4000 }))

    const wrapper = mount(ReaderPage)
    await settle()

    const container = await getMarkdownBody(wrapper)
    const expected = Math.round(0.5 * (MOCK_HEIGHT - MOCK_VIEWPORT))
    expect(container!.scrollTop).toBe(expected)
    wrapper.unmount()
  })

  it('saves position on scroll after debounce', async () => {
    setupStoreReady()
    const wrapper = mount(ReaderPage)
    await settle()

    const container = await getMarkdownBody(wrapper)
    container!.scrollTop = 600
    container!.dispatchEvent(new Event('scroll'))
    // 防抖 500ms
    await new Promise((r) => setTimeout(r, 700))

    const records = JSON.parse(localStorage.getItem('featherview.readingPositions.v1') ?? '[]')
    expect(records.length).toBe(1)
    expect(records[0].scrollTop).toBe(600)
    expect(records[0].scrollRatio).toBeCloseTo(600 / (MOCK_HEIGHT - MOCK_VIEWPORT))
    wrapper.unmount()
  })

  it('saves position before switching file', async () => {
    const store = setupStoreReady()
    const wrapper = mount(ReaderPage)
    await settle()

    const container = await getMarkdownBody(wrapper)
    container!.scrollTop = 300
    container!.dispatchEvent(new Event('scroll'))

    // 切换文件（open 会先设置新 source，触发旧位置保存）
    store.source = { id: 't2', name: 'b.md', extension: 'md', path: 'C:/docs/b.md' }
    await new Promise((r) => setTimeout(r, 100))

    const records = JSON.parse(localStorage.getItem('featherview.readingPositions.v1') ?? '[]')
    expect(records.some((r: { path?: string }) => r.path === MD_PATH)).toBe(true)
    wrapper.unmount()
  })

  it('sets window title to file name', async () => {
    setupStoreReady()
    const wrapper = mount(ReaderPage)
    await settle()
    expect(document.title).toBe('readme.md — FeatherView')
    wrapper.unmount()
  })

  it('ignores duplicate open while loading', async () => {
    const store = useDocumentStore()
    store.status = 'loading'
    const ok = await store.open(makeMeta().source)
    expect(ok).toBe(false)
  })
})
