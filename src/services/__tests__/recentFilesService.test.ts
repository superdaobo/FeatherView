import { describe, expect, it } from 'vitest'
import {
  addRecentFile,
  clearRecentFiles,
  loadRecentFiles,
  MAX_RECENT_FILES,
  removeRecentFile,
} from '../recentFilesService'

function entry(path: string, size = 100) {
  return { path, name: path.split(/[\\/]/).pop() ?? path, extension: 'md', size }
}

describe('recentFilesService', () => {
  it('sorts by lastOpenedAt descending and dedupes by path', () => {
    let files = addRecentFile([], entry('a.md'))
    files = addRecentFile(files, entry('b.md'))
    // 再次打开 a.md：应排到最前且只保留一条
    files = addRecentFile(files, entry('a.md'))

    expect(files.length).toBe(2)
    expect(files[0].path).toBe('a.md')
    expect(files[1].path).toBe('b.md')
    expect(files[0].lastOpenedAt).toBeGreaterThanOrEqual(files[1].lastOpenedAt)
  })

  it('keeps at most 20 entries', () => {
    let files: ReturnType<typeof loadRecentFiles> = []
    for (let i = 0; i < 25; i++) {
      files = addRecentFile(files, entry(`file-${i}.md`))
    }
    expect(files.length).toBe(MAX_RECENT_FILES)
    expect(files[0].path).toBe('file-24.md')
  })

  it('removes a single entry', () => {
    let files = addRecentFile(addRecentFile([], entry('a.md')), entry('b.md'))
    files = removeRecentFile(files, 'a.md')
    expect(files.map((f) => f.path)).toEqual(['b.md'])
  })

  it('clears all entries', () => {
    addRecentFile([], entry('a.md'))
    const files = clearRecentFiles()
    expect(files).toEqual([])
  })

  it('persists to localStorage and loads back', () => {
    // 链式调用保持记录累积
    addRecentFile(addRecentFile([], entry('persist.md', 42)), entry('second.md'))
    const loaded = loadRecentFiles()
    expect(loaded.length).toBe(2)
    expect(loaded.find((f) => f.path === 'persist.md')?.size).toBe(42)
  })

  it('ignores corrupted storage', () => {
    localStorage.setItem('featherview.recentFiles', '{not json')
    expect(loadRecentFiles()).toEqual([])
  })
})
