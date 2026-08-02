/**
 * 收藏服务：localStorage 持久化，上限 100 条，按 addedAt 降序。
 * 数据损坏 / 非法值自动过滤，任何解析失败都回退为空列表（不抛异常）。
 */
import type { FavoriteItem } from '../types/nightly'

const STORAGE_KEY = 'featherview.favorites.v1'
export const MAX_FAVORITES = 100

/** 校验并清洗单条收藏；非法返回 null */
export function sanitizeFavorite(value: unknown): FavoriteItem | null {
  if (!value || typeof value !== 'object') return null
  const f = value as Record<string, unknown>
  if (typeof f.path !== 'string' || !f.path) return null
  if (typeof f.name !== 'string' || !f.name) return null
  if (typeof f.addedAt !== 'number' || !Number.isFinite(f.addedAt)) return null
  return {
    path: f.path,
    name: f.name,
    extension: typeof f.extension === 'string' ? f.extension : '',
    size: typeof f.size === 'number' && Number.isFinite(f.size) ? f.size : undefined,
    addedAt: f.addedAt,
  }
}

/** 读取全部收藏（按 addedAt 降序，超上限截断） */
export function loadFavorites(): FavoriteItem[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed
      .map(sanitizeFavorite)
      .filter((f): f is FavoriteItem => f !== null)
      .sort((a, b) => b.addedAt - a.addedAt)
      .slice(0, MAX_FAVORITES)
  } catch {
    return []
  }
}

function persist(items: FavoriteItem[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(items.slice(0, MAX_FAVORITES)))
  } catch {
    // 存储失败静默（隐私模式等）
  }
}

/** 添加收藏（同 path 更新 addedAt 置顶；超上限淘汰最旧） */
export function addFavorite(item: FavoriteItem): FavoriteItem[] {
  const next = [item, ...loadFavorites().filter((f) => f.path !== item.path)]
    .sort((a, b) => b.addedAt - a.addedAt)
    .slice(0, MAX_FAVORITES)
  persist(next)
  return next
}

/** 移除收藏 */
export function removeFavorite(path: string): FavoriteItem[] {
  const next = loadFavorites().filter((f) => f.path !== path)
  persist(next)
  return next
}

/** 是否已收藏 */
export function isFavorite(path: string): boolean {
  return loadFavorites().some((f) => f.path === path)
}

/** 清空全部收藏 */
export function clearFavorites(): FavoriteItem[] {
  persist([])
  return []
}