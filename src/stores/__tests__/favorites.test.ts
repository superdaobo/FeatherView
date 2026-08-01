import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it } from 'vitest'
import { useFavoritesStore } from '../favorites'
import type { DocumentSource } from '../../types'

function makeSource(name: string): DocumentSource {
  return { id: name, name, extension: 'md', path: `C:/docs/${name}` }
}

describe('favorites store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
  })

  it('toggles favorite on and off', () => {
    const store = useFavoritesStore()
    expect(store.isStarred(makeSource('a.md'))).toBe(false)
    const added = store.toggle(makeSource('a.md'))
    expect(added).toBe(true)
    expect(store.items.length).toBe(1)
    expect(store.isStarred(makeSource('a.md'))).toBe(true)
    const removed = store.toggle(makeSource('a.md'))
    expect(removed).toBe(false)
    expect(store.items.length).toBe(0)
  })

  it('persists favorites across store instances', () => {
    useFavoritesStore().toggle(makeSource('a.md'))
    const other = useFavoritesStore()
    expect(other.items.length).toBe(1)
    expect(other.items[0].name).toBe('a.md')
  })

  it('removes and clears all', () => {
    const store = useFavoritesStore()
    store.toggle(makeSource('a.md'))
    store.toggle(makeSource('b.md'))
    store.remove('C:/docs/a.md')
    expect(store.items.length).toBe(1)
    store.clearAll()
    expect(store.items.length).toBe(0)
  })
})