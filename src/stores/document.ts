/**
 * 当前文档 store：加载状态机与错误管理。
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
    close,
    setError,
  }
})
