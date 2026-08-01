/**
 * 阅读位置服务：localStorage 持久化，最多 150 条，按 updatedAt 淘汰最旧。
 * 数据损坏/非法值自动过滤，任何解析失败都回退为空记录（不抛异常、不白屏）。
 */
import type { ReadingPositionRecord } from '../types'

const STORAGE_KEY = 'featherview.readingPositions.v1'
export const MAX_POSITIONS = 150

/** 校验并清洗单条记录；非法返回 null */
export function sanitizeRecord(value: unknown): ReadingPositionRecord | null {
  if (!value || typeof value !== 'object') return null
  const r = value as Record<string, unknown>
  if (typeof r.sourceId !== 'string' || !r.sourceId) return null
  if (typeof r.updatedAt !== 'number' || !Number.isFinite(r.updatedAt)) return null
  if (typeof r.rendererId !== 'string') return null

  const isFiniteNonNegative = (v: unknown): v is number =>
    typeof v === 'number' && Number.isFinite(v) && v >= 0

  if (!isFiniteNonNegative(r.scrollTop)) return null
  if (!isFiniteNonNegative(r.scrollRatio) || r.scrollRatio > 1) return null
  if (!isFiniteNonNegative(r.viewportHeight)) return null
  if (!isFiniteNonNegative(r.contentHeight)) return null

  const record: ReadingPositionRecord = {
    sourceId: r.sourceId,
    path: typeof r.path === 'string' ? r.path : undefined,
    uri: typeof r.uri === 'string' ? r.uri : undefined,
    rendererId: r.rendererId,
    scrollTop: r.scrollTop,
    scrollRatio: r.scrollRatio,
    viewportHeight: r.viewportHeight,
    contentHeight: r.contentHeight,
    headingId: typeof r.headingId === 'string' ? r.headingId : undefined,
    updatedAt: r.updatedAt,
  }
  return record
}

/** 读取全部记录（按 updatedAt 降序），数据损坏时回退空数组 */
export function loadPositions(): ReadingPositionRecord[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed
      .map(sanitizeRecord)
      .filter((r): r is ReadingPositionRecord => r !== null)
      .sort((a, b) => b.updatedAt - a.updatedAt)
      .slice(0, MAX_POSITIONS)
  } catch {
    return []
  }
}

function persist(records: ReadingPositionRecord[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(records.slice(0, MAX_POSITIONS)))
  } catch {
    // 存储失败静默（隐私模式等）
  }
}

/** 读取单条记录 */
export function getPosition(sourceId: string): ReadingPositionRecord | null {
  const records = loadPositions()
  return records.find((r) => r.sourceId === sourceId) ?? null
}

/** 保存/更新记录（按 sourceId 去重；超过上限淘汰最旧） */
export function savePosition(record: ReadingPositionRecord): void {
  const records = loadPositions()
  const next = [record, ...records.filter((r) => r.sourceId !== record.sourceId)]
    .sort((a, b) => b.updatedAt - a.updatedAt)
    .slice(0, MAX_POSITIONS)
  persist(next)
}

/** 删除单条记录 */
export function removePosition(sourceId: string): void {
  persist(loadPositions().filter((r) => r.sourceId !== sourceId))
}

/** 清空全部阅读位置（不影响最近文件与其他设置） */
export function clearPositions(): void {
  persist([])
}

/** 将滚动位置限制在有效范围内 */
export function clampScrollTop(scrollTop: number, max: number): number {
  if (!Number.isFinite(scrollTop) || scrollTop < 0) return 0
  if (!Number.isFinite(max) || max < 0) return 0
  return Math.min(scrollTop, max)
}
