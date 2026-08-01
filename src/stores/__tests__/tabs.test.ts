import { flushPromises } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useTabsStore, MAX_TABS } from '../tabs'
import { useDocumentStore } from '../document'
import { getDocumentIdentity } from '../../services/platformService'
import { savePosition } from '../../services/readingPositionService'
import type { DocumentSource } from '../../types'

// mock tauri invoke（documentStore.open → documentService.openDocument）
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))

function makeSource(name: string, path = `C:/docs/${name}`): DocumentSource {
  return { id: `id-${name}`, name, extension: 'md', path }
}

function setupInvoke(): void {
  invokeMock.mockReset()
  invokeMock.mockImplementation(async (cmd: string) => {
    if (cmd === 'file_metadata') return { size: 100, modifiedAt: undefined, isFile: true }
    if (cmd === 'read_file') return { content: `内容 ${Math.random()}`, size: 100, isBinary: false }
    throw new Error(`unknown command: ${cmd}`)
  })
}

describe('tabs store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
    // 让 platformService.isTauri() 返回 true（documentService 路径可走）
    ;(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {}
    setupInvoke()
  })

  it('opens a tab, activates it and loads the document', async () => {
    const tabs = useTabsStore()
    const tab = await tabs.openTab(makeSource('a.md'))
    await flushPromises()
    expect(tabs.tabs.length).toBe(1)
    expect(tabs.activeTabId).toBe(tab.tabId)
    expect(useDocumentStore().status).toBe('ready')
  })

  it('reuses the existing tab for the same file (dedupe, no re-read)', async () => {
    const tabs = useTabsStore()
    const a = await tabs.openTab(makeSource('a.md'))
    await flushPromises()
    const calls = invokeMock.mock.calls.length
    const again = await tabs.openTab(makeSource('a.md'))
    await flushPromises()
    expect(again.tabId).toBe(a.tabId)
    expect(tabs.tabs.length).toBe(1)
    expect(invokeMock.mock.calls.length).toBe(calls)
  })

  it('switches tabs via snapshot without re-reading', async () => {
    const tabs = useTabsStore()
    await tabs.openTab(makeSource('a.md'))
    await flushPromises()
    await tabs.openTab(makeSource('b.md'))
    await flushPromises()
    const calls = invokeMock.mock.calls.length
    await tabs.activateTab(tabs.tabs[0].tabId)
    await flushPromises()
    expect(invokeMock.mock.calls.length).toBe(calls)
    expect(useDocumentStore().source?.name).toBe('a.md')
    expect(tabs.activeTabId).toBe(tabs.tabs[0].tabId)
  })

  it('evicts oldest non-active tab when exceeding the limit', async () => {
    const tabs = useTabsStore()
    for (let i = 0; i < MAX_TABS + 2; i++) {
      await tabs.openTab(makeSource(`f${i}.md`))
      await flushPromises()
    }
    expect(tabs.tabs.length).toBe(MAX_TABS)
    expect(tabs.tabs.some((t) => t.source.name === 'f0.md')).toBe(false)
    expect(tabs.tabs.some((t) => t.source.name === 'f1.md')).toBe(false)
    expect(tabs.tabs.some((t) => t.source.name === 'f2.md')).toBe(true)
  })

  it('activates the right neighbor when closing the active tab', async () => {
    const tabs = useTabsStore()
    const a = await tabs.openTab(makeSource('a.md'))
    await flushPromises()
    const b = await tabs.openTab(makeSource('b.md'))
    await flushPromises()
    await tabs.openTab(makeSource('c.md'))
    await flushPromises()
    await tabs.activateTab(a.tabId)
    await tabs.closeTab(a.tabId)
    expect(tabs.activeTabId).toBe(b.tabId)
    expect(tabs.tabs.map((t) => t.source.name)).toEqual(['b.md', 'c.md'])
  })

  it('clears document store when the last tab is closed', async () => {
    const tabs = useTabsStore()
    await tabs.openTab(makeSource('a.md'))
    await flushPromises()
    await tabs.closeTab(tabs.activeTabId as string)
    expect(tabs.tabs.length).toBe(0)
    expect(tabs.activeTabId).toBeNull()
    expect(useDocumentStore().status).toBe('idle')
  })

  it('closes other tabs keeping the target active', async () => {
    const tabs = useTabsStore()
    const a = await tabs.openTab(makeSource('a.md'))
    await flushPromises()
    await tabs.openTab(makeSource('b.md'))
    await flushPromises()
    await tabs.closeOtherTabs(a.tabId)
    expect(tabs.tabs.map((t) => t.source.name)).toEqual(['a.md'])
    expect(tabs.activeTabId).toBe(a.tabId)
    await flushPromises()
    expect(useDocumentStore().source?.name).toBe('a.md')
  })

  it('remembers the reading position on the tab when switching', async () => {
    const tabs = useTabsStore()
    const a = await tabs.openTab(makeSource('a.md'))
    await flushPromises()
    const documentStore = useDocumentStore()
    savePosition({
      sourceId: getDocumentIdentity(documentStore.meta!.source),
      path: 'C:/docs/a.md',
      rendererId: 'markdown',
      scrollTop: 100,
      scrollRatio: 0.5,
      viewportHeight: 800,
      contentHeight: 2000,
      updatedAt: Date.now(),
    })
    await tabs.openTab(makeSource('b.md'))
    await flushPromises()
    expect(a.position?.scrollTop).toBe(100)
  })
})