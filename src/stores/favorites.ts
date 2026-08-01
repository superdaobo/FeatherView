/**
 * 收藏 store：首页收藏区与阅读页星标共用。
 */
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { DocumentSource } from '../types'
import type { FavoriteItem } from '../types/nightly'
import {
  addFavorite,
  clearFavorites,
  isFavorite,
  loadFavorites,
  removeFavorite,
} from '../services/favoritesService'

export const useFavoritesStore = defineStore('favorites', () => {
  const items = ref<FavoriteItem[]>(loadFavorites())

  function toItem(source: DocumentSource): FavoriteItem {
    return {
      path: source.path ?? source.uri ?? source.id,
      name: source.name,
      extension: source.extension,
      size: source.size,
      addedAt: Date.now(),
    }
  }

  /** 切换收藏状态；返回是否已收藏 */
  function toggle(source: DocumentSource): boolean {
    const item = toItem(source)
    if (isFavorite(item.path)) {
      items.value = removeFavorite(item.path)
      return false
    }
    items.value = addFavorite(item)
    return true
  }

  function remove(path: string): void {
    items.value = removeFavorite(path)
  }

  function isStarred(source: DocumentSource): boolean {
    return isFavorite(source.path ?? source.uri ?? source.id)
  }

  function clearAll(): void {
    items.value = clearFavorites()
  }

  return { items, toggle, remove, isStarred, clearAll }
})