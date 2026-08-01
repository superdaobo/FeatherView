/**
 * 最近文件服务：localStorage 持久化，最多 20 条，按最近打开时间排序去重。
 */
import type { RecentFile } from '../types'

const STORAGE_KEY = 'featherview.recentFiles'
export const MAX_RECENT_FILES = 20

export function loadRecentFiles(): RecentFile[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed
      .filter((item): item is RecentFile => {
        return (
          item &&
          typeof item.path === 'string' &&
          typeof item.name === 'string' &&
          typeof item.lastOpenedAt === 'number'
        )
      })
      .sort((a, b) => b.lastOpenedAt - a.lastOpenedAt)
      .slice(0, MAX_RECENT_FILES)
  } catch {
    return []
  }
}

function saveRecentFiles(files: RecentFile[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(files))
  } catch {
    // 存储失败时静默（隐私模式等场景）
  }
}

/**
 * 添加或更新一条记录：按 path 去重，更新打开时间，最多保留 20 条。
 */
export function addRecentFile(files: RecentFile[], entry: Omit<RecentFile, 'lastOpenedAt'>): RecentFile[] {
  const now = Date.now()
  const record: RecentFile = { ...entry, lastOpenedAt: now }
  const withoutOld = files.filter((f) => f.path !== record.path)
  const next = [record, ...withoutOld].slice(0, MAX_RECENT_FILES)
  saveRecentFiles(next)
  return next
}

export function removeRecentFile(files: RecentFile[], path: string): RecentFile[] {
  const next = files.filter((f) => f.path !== path)
  saveRecentFiles(next)
  return next
}

export function clearRecentFiles(): RecentFile[] {
  saveRecentFiles([])
  return []
}
