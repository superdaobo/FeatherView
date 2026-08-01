import { beforeEach, describe, expect, it } from 'vitest'
import {
  addFavorite,
  clearFavorites,
  isFavorite,
  loadFavorites,
  MAX_FAVORITES,
  removeFavorite,
  sanitizeFavorite,
} from '../favoritesService'
import type { FavoriteItem } from '../../types/nightly'

function makeItem(path: string, addedAt = Date.now()): FavoriteItem {
  return { path, name: path.split('/').pop() ?? path, extension: 'md', addedAt }
}

describe('favoritesService', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('adds, dedupes and removes favorites', () => {
    addFavorite(makeItem('C:/a.md', 1))
    addFavorite(makeItem('C:/b.md', 2))
    addFavorite(makeItem('C:/a.md', 3))
    const items = loadFavorites()
    expect(items.length).toBe(2)
    // 重复添加同 path → 更新置顶
    expect(items[0].path).toBe('C:/a.md')
    expect(isFavorite('C:/a.md')).toBe(true)
    removeFavorite('C:/a.md')
    expect(isFavorite('C:/a.md')).toBe(false)
  })

  it('sanitizes invalid records', () => {
    expect(sanitizeFavorite(null)).toBeNull()
    expect(sanitizeFavorite({ path: 'C:/x.md', name: 'x.md', addedAt: 'bad' })).toBeNull()
    const clean = sanitizeFavorite({ path: 'C:/x.md', name: 'x.md', addedAt: 1, extension: 42 })
    expect(clean).not.toBeNull()
    expect(clean!.extension).toBe('')
  })

  it('caps at MAX_FAVORITES keeping the newest', () => {
    for (let i = 0; i < MAX_FAVORITES + 5; i++) {
      addFavorite(makeItem(`C:/f${i}.md`, i))
    }
    const items = loadFavorites()
    expect(items.length).toBe(MAX_FAVORITES)
    expect(items[0].path).toBe(`C:/f${MAX_FAVORITES + 4}.md`)
  })

  it('clears all favorites', () => {
    addFavorite(makeItem('C:/a.md'))
    clearFavorites()
    expect(loadFavorites().length).toBe(0)
  })
})