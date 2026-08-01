import { beforeEach, describe, expect, it } from 'vitest'
import {
  clampScrollTop,
  clearPositions,
  getPosition,
  loadPositions,
  MAX_POSITIONS,
  removePosition,
  sanitizeRecord,
  savePosition,
} from '../readingPositionService'
import type { ReadingPositionRecord } from '../../types'

function makeRecord(sourceId: string, overrides: Partial<ReadingPositionRecord> = {}): ReadingPositionRecord {
  return {
    sourceId,
    path: `C:\\docs\\${sourceId}.md`,
    rendererId: 'markdown',
    scrollTop: 500,
    scrollRatio: 0.5,
    viewportHeight: 800,
    contentHeight: 1800,
    updatedAt: Date.now(),
    ...overrides,
  }
}

describe('readingPositionService', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('saves and reads a record', () => {
    savePosition(makeRecord('a.md'))
    const record = getPosition('a.md')
    expect(record).not.toBeNull()
    expect(record!.scrollRatio).toBe(0.5)
  })

  it('updates the same file record', () => {
    savePosition(makeRecord('a.md', { scrollTop: 100, updatedAt: 1000 }))
    savePosition(makeRecord('a.md', { scrollTop: 900, updatedAt: 2000 }))
    const records = loadPositions()
    expect(records.length).toBe(1)
    expect(records[0].scrollTop).toBe(900)
  })

  it('removes a single record', () => {
    savePosition(makeRecord('a.md'))
    savePosition(makeRecord('b.md'))
    removePosition('a.md')
    expect(getPosition('a.md')).toBeNull()
    expect(getPosition('b.md')).not.toBeNull()
  })

  it('clears all records', () => {
    savePosition(makeRecord('a.md'))
    clearPositions()
    expect(loadPositions()).toEqual([])
  })

  it('recovers from corrupted json', () => {
    localStorage.setItem('featherview.readingPositions.v1', '{not json')
    expect(loadPositions()).toEqual([])
    expect(getPosition('a.md')).toBeNull()
  })

  it('filters NaN and invalid values', () => {
    const good = makeRecord('good.md')
    localStorage.setItem(
      'featherview.readingPositions.v1',
      JSON.stringify([
        good,
        { ...makeRecord('bad1.md'), scrollTop: NaN },
        { ...makeRecord('bad2.md'), scrollRatio: -1 },
        { ...makeRecord('bad3.md'), scrollRatio: 2 },
        { ...makeRecord('bad4.md'), updatedAt: 'x' },
        'garbage',
        null,
      ]),
    )
    const records = loadPositions()
    expect(records.length).toBe(1)
    expect(records[0].sourceId).toBe('good.md')
  })

  it('evicts oldest records beyond limit', () => {
    for (let i = 0; i < MAX_POSITIONS + 10; i++) {
      savePosition(makeRecord(`file-${i}.md`, { updatedAt: 1000 + i }))
    }
    const records = loadPositions()
    expect(records.length).toBe(MAX_POSITIONS)
    // 最新的在前（updatedAt 大）
    expect(records[0].sourceId).toBe(`file-${MAX_POSITIONS + 9}.md`)
    expect(getPosition('file-0.md')).toBeNull()
  })

  it('sanitizeRecord rejects missing fields', () => {
    expect(sanitizeRecord({})).toBeNull()
    expect(sanitizeRecord(makeRecord('x.md'))).not.toBeNull()
    const r = sanitizeRecord(makeRecord('x.md', { headingId: undefined }))
    expect(r!.headingId).toBeUndefined()
  })

  it('clampScrollTop clamps to valid range', () => {
    expect(clampScrollTop(-5, 1000)).toBe(0)
    expect(clampScrollTop(1500, 1000)).toBe(1000)
    expect(clampScrollTop(500, 1000)).toBe(500)
    expect(clampScrollTop(NaN, 1000)).toBe(0)
    expect(clampScrollTop(500, NaN)).toBe(0)
  })

  it('does not fail when disabled (no writes)', () => {
    // 关闭功能 = 调用方不再 savePosition；此处验证服务在空状态下稳定
    clearPositions()
    expect(loadPositions()).toEqual([])
  })
})
