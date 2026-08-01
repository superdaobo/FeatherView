/**
 * 最近文件 store。
 */
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { DocumentSource, RecentFile } from '../types'
import {
  addRecentFile,
  clearRecentFiles,
  loadRecentFiles,
  removeRecentFile,
} from '../services/recentFilesService'

export const useRecentFilesStore = defineStore('recentFiles', () => {
  const files = ref<RecentFile[]>(loadRecentFiles())

  /** 打开文档后记录（source 可能来自拖拽/选择/最近文件） */
  function recordOpen(source: DocumentSource): void {
    files.value = addRecentFile(files.value, {
      path: source.path ?? source.uri ?? '',
      name: source.name,
      extension: source.extension,
      size: source.size,
    })
  }

  function remove(path: string): void {
    files.value = removeRecentFile(files.value, path)
  }

  function clearAll(): void {
    files.value = clearRecentFiles()
  }

  /** 将最近记录转为 DocumentSource（用于重新打开） */
  function toSource(file: RecentFile): DocumentSource {
    return {
      id: `recent:${file.path}`,
      name: file.name,
      extension: file.extension,
      path: file.path,
      size: file.size,
    }
  }

  return { files, recordOpen, remove, clearAll, toSource }
})
