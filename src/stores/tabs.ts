/**
 * 多标签页 store（合同第 6 节）。
 *
 * - tabs：标签列表（上限 MAX_TABS，超出淘汰最旧非激活）
 * - documentStore 保持为"当前激活标签的视图状态"
 * - 每个标签的加载快照（meta + result）缓存在 store 内：同源标签切换不重新读取
 * - 阅读位置：切换/关闭前把当前文档位置写入 tab.position（从 readingPositionService 读取），
 *   激活时再把 tab.position 写回 localStorage，由 useReadingPosition 按既有逻辑恢复
 */
import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { DocumentMeta, DocumentSource, ReadFileResult } from '../types'
import type { TabState } from '../types/nightly'
import { getDocumentIdentity } from '../services/platformService'
import { getPosition, savePosition } from '../services/readingPositionService'
import { useDocumentStore } from './document'

export const MAX_TABS = 16

export const useTabsStore = defineStore('tabs', () => {
  const tabs = ref<TabState[]>([])
  const activeTabId = ref<string | null>(null)

  /** tabId → 加载快照（避免标签切换时同文件重读） */
  const snapshots = new Map<string, { meta: DocumentMeta; result: ReadFileResult }>()

  const activeTab = computed(() => tabs.value.find((t) => t.tabId === activeTabId.value) ?? null)
  const count = computed(() => tabs.value.length)

  function nextTabId(): string {
    return `tab-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  }

  function findTabBySource(source: DocumentSource): TabState | undefined {
    const identity = getDocumentIdentity(source)
    return tabs.value.find((t) => getDocumentIdentity(t.source) === identity)
  }

  /** 把当前激活文档的最新阅读位置写入 tab.position（从 localStorage 读取） */
  function saveActiveTabPosition(): void {
    const tab = activeTab.value
    const documentStore = useDocumentStore()
    if (!tab || !documentStore.meta) return
    const record = getPosition(getDocumentIdentity(documentStore.meta.source))
    if (record) tab.position = record
  }

  /** 激活前把标签记忆的阅读位置写回 localStorage，供 useReadingPosition 恢复 */
  function preparePositionRestore(tab: TabState): void {
    if (tab.position) {
      savePosition({ ...tab.position, updatedAt: Date.now() })
    }
  }

  function removeTabState(tabId: string): void {
    tabs.value = tabs.value.filter((t) => t.tabId !== tabId)
    snapshots.delete(tabId)
  }

  /** 让 documentStore 呈现目标标签：有快照直接恢复（不重读），否则真实加载并缓存 */
  function applyTabToDocument(tab: TabState): void {
    const documentStore = useDocumentStore()
    const snapshot = snapshots.get(tab.tabId)
    if (snapshot) {
      documentStore.restoreSnapshot(tab.source, snapshot.meta, snapshot.result)
      tab.meta = snapshot.meta
      return
    }
    void documentStore.open(tab.source).then((ok) => {
      if (ok && documentStore.meta && documentStore.result) {
        snapshots.set(tab.tabId, { meta: documentStore.meta, result: documentStore.result })
        tab.meta = documentStore.meta
      }
    })
  }

  async function activateTab(tabId: string): Promise<void> {
    const tab = tabs.value.find((t) => t.tabId === tabId)
    if (!tab) return
    if (tabId === activeTabId.value) return
    saveActiveTabPosition()
    activeTabId.value = tabId
    preparePositionRestore(tab)
    applyTabToDocument(tab)
  }

  /** 超出上限时淘汰最旧非激活标签（全部激活时淘汰最旧） */
  function evictIfFull(): void {
    if (tabs.value.length < MAX_TABS) return
    const candidates = tabs.value.filter((t) => t.tabId !== activeTabId.value)
    const evict = candidates[0] ?? tabs.value[0]
    if (evict) removeTabState(evict.tabId)
  }

  /** 打开标签：同源去重（激活已有），否则新建并激活 */
  async function openTab(source: DocumentSource): Promise<TabState> {
    const existing = findTabBySource(source)
    if (existing) {
      await activateTab(existing.tabId)
      return existing
    }
    evictIfFull()
    const tab: TabState = { tabId: nextTabId(), source }
    tabs.value.push(tab)
    await activateTab(tab.tabId)
    return tab
  }

  /** 关闭标签；关闭激活标签时激活相邻（右侧优先，否则左侧）；全关则清空 documentStore */
  async function closeTab(tabId: string): Promise<void> {
    const isActive = tabId === activeTabId.value
    saveActiveTabPosition()
    let nextId: string | null = null
    if (isActive) {
      const index = tabs.value.findIndex((t) => t.tabId === tabId)
      nextId = tabs.value[index + 1]?.tabId ?? tabs.value[index - 1]?.tabId ?? null
    }
    removeTabState(tabId)
    if (!isActive) return
    if (nextId) {
      const next = tabs.value.find((t) => t.tabId === nextId)
      if (next) {
        activeTabId.value = nextId
        preparePositionRestore(next)
        applyTabToDocument(next)
      }
    } else {
      activeTabId.value = null
      useDocumentStore().close()
    }
  }

  /** 关闭除指定标签外的所有标签（保留目标激活；目标未激活时激活之） */
  async function closeOtherTabs(tabId: string): Promise<void> {
    const keep = tabs.value.find((t) => t.tabId === tabId)
    if (!keep) return
    saveActiveTabPosition()
    for (const t of tabs.value) {
      if (t.tabId !== tabId) snapshots.delete(t.tabId)
    }
    tabs.value = [keep]
    if (activeTabId.value !== tabId) {
      activeTabId.value = tabId
      preparePositionRestore(keep)
      applyTabToDocument(keep)
    }
  }

  /** 全部关闭（返回首页由 ReaderPage 监听 activeTabId 处理） */
  async function closeAllTabs(): Promise<void> {
    saveActiveTabPosition()
    snapshots.clear()
    tabs.value = []
    activeTabId.value = null
    useDocumentStore().close()
  }

  return {
    tabs,
    activeTabId,
    activeTab,
    count,
    openTab,
    activateTab,
    closeTab,
    closeOtherTabs,
    closeAllTabs,
  }
})