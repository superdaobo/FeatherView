/**
 * 当前文档 store：加载状态机与错误管理。
 * documentStore 保持为"当前激活标签的视图状态"（多标签架构，见 stores/tabs.ts）。
 */
import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { AppErrorInfo, DocumentMeta, DocumentSource, DocumentStatus, ReadFileResult } from '../types'
import { getErrorInfo, openDocument } from '../services/documentService'
import { useRecentFilesStore } from './recentFiles'

export const useDocumentStore = defineStore('document', () => {
  const status = ref<DocumentStatus>('idle')
  const source = ref<DocumentSource | null>(null)
  const meta = ref<DocumentMeta | null>(null)
  const result = ref<ReadFileResult | null>(null)
  const error = ref<AppErrorInfo | null>(null)

  const isReady = computed(() => status.value === 'ready')
  const isLoading = computed(() => status.value === 'loading')

  /** 打开文档：统一入口，所有打开方式都汇聚到这里 */
  async function open(sourceInput: DocumentSource): Promise<boolean> {
    // 加载中忽略重复打开请求（防止连点/事件竞态）
    if (status.value === 'loading') return false
    status.value = 'loading'
    source.value = sourceInput
    meta.value = null
    result.value = null
    error.value = null
    try {
      const { meta: m, result: r } = await openDocument(sourceInput)
      meta.value = m
      result.value = r
      status.value = 'ready'

      // 记录最近文件
      const recent = useRecentFilesStore()
      recent.recordOpen(sourceInput)
      return true
    } catch (err) {
      const info = getErrorInfo(err)
      error.value = info
      status.value = 'error'
      console.error(`[FeatherView] 打开文档失败: ${sourceInput.path ?? sourceInput.uri}`, info)
      return false
    }
  }

  /** 重新加载当前文档 */
  async function reload(): Promise<boolean> {
    if (!source.value) return false
    return open(source.value)
  }

  /** 从标签快照恢复视图状态（不重新读取文件；多标签切换专用） */
  function restoreSnapshot(src: DocumentSource, m: DocumentMeta, r: ReadFileResult): void {
    status.value = 'ready'
    source.value = src
    meta.value = m
    result.value = r
    error.value = null
  }

  function close(): void {
    status.value = 'idle'
    source.value = null
    meta.value = null
    result.value = null
    error.value = null
  }

  function setError(info: AppErrorInfo): void {
    error.value = info
    status.value = 'error'
  }

  return {
    status,
    source,
    meta,
    result,
    error,
    isReady,
    isLoading,
    open,
    reload,
    restoreSnapshot,
    close,
    setError,
  }
})