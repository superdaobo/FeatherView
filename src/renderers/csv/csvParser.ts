/**
 * 轻量 CSV 解析器（纯函数，零第三方依赖）。
 *
 * 支持：
 * - 分隔符自动检测：逗号 / 分号 / 制表符（统计前 8KB 中引号外的候选符出现次数）
 * - 双引号字段、引号内转义（"" → "）、引号内换行
 * - BOM（\uFEFF）剥离
 * - 行数截断保护（默认最多渲染 5000 行数据，避免大文件卡死 DOM）
 * - 宽容模式：未闭合引号 / 字段数不一致不抛错，通过 issues 提示
 */

export type CsvDelimiter = ',' | ';' | '\t'

export interface CsvParseResult {
  /** 表头行字段（第一行），空文件为 [] */
  header: string[]
  /** 数据行（不含表头） */
  rows: string[][]
  /** 检测到的分隔符 */
  delimiter: CsvDelimiter
  /** 是否因超过行数上限被截断 */
  truncated: boolean
  /** 估算的总行数（含表头；截断时为近似值） */
  estimatedTotalRows: number
  /** 解析中遇到的问题（宽容处理，不抛错） */
  issues: string[]
}

/** 最多渲染的数据行数（不含表头），超出截断 */
export const MAX_CSV_ROWS = 5000

/** 分隔符检测的采样字符数 */
const DETECT_SAMPLE_CHARS = 8192

const DELIMITER_CANDIDATES: CsvDelimiter[] = [',', ';', '\t']

/**
 * 检测分隔符：统计样本（前 8KB）中引号外各候选符出现次数，取最多者。
 * 平局时按候选顺序（逗号 > 分号 > 制表符）优先；全部为 0 时默认逗号。
 */
export function detectDelimiter(text: string): CsvDelimiter {
  const sample = text.slice(0, DETECT_SAMPLE_CHARS)
  const counts: Record<CsvDelimiter, number> = { ',': 0, ';': 0, '\t': 0 }
  let inQuotes = false
  for (let i = 0; i < sample.length; i++) {
    const ch = sample[i]
    if (ch === '"') {
      // 引号内转义 "" 不算状态切换
      if (inQuotes && sample[i + 1] === '"') {
        i += 1
        continue
      }
      inQuotes = !inQuotes
      continue
    }
    if (!inQuotes && (ch === ',' || ch === ';' || ch === '\t')) {
      counts[ch] += 1
    }
  }
  let best: CsvDelimiter = ','
  let bestCount = -1
  for (const d of DELIMITER_CANDIDATES) {
    if (counts[d] > bestCount) {
      best = d
      bestCount = counts[d]
    }
  }
  return best
}

/** 估算剩余文本的换行数（截断时用于近似总行数） */
function countNewlines(text: string, from: number): number {
  let n = 0
  for (let i = from; i < text.length; i++) {
    if (text[i] === '\n') n += 1
  }
  return n
}

/**
 * 解析 CSV 文本。永远不抛异常：损坏的输入以宽容方式解析并在 issues 中说明。
 */
export function parseCsv(text: string): CsvParseResult {
  // 剥离 BOM
  let src = text
  if (src.charCodeAt(0) === 0xfeff) src = src.slice(1)

  if (src.length === 0) {
    return {
      header: [],
      rows: [],
      delimiter: ',',
      truncated: false,
      estimatedTotalRows: 0,
      issues: [],
    }
  }

  const delimiter = detectDelimiter(src)
  const rows: string[][] = []
  const issues: string[] = []

  let row: string[] = []
  let field = ''
  let inQuotes = false
  let stopAt = -1
  let truncated = false

  let i = 0
  const n = src.length
  // 逐字符状态机（index 循环以便 peek 处理转义引号与 \r\n）
  while (i < n) {
    const ch = src[i]
    if (inQuotes) {
      if (ch === '"') {
        if (src[i + 1] === '"') {
          field += '"'
          i += 2
          continue
        }
        inQuotes = false
        i += 1
        continue
      }
      field += ch
      i += 1
      continue
    }
    if (ch === '"' && field.length === 0) {
      // 字段开头的引号进入引号模式
      inQuotes = true
      i += 1
      continue
    }
    if (ch === delimiter) {
      row.push(field)
      field = ''
      i += 1
      continue
    }
    if (ch === '\n' || ch === '\r') {
      row.push(field)
      field = ''
      rows.push(row)
      row = []
      // \r\n 视为单个换行
      if (ch === '\r' && src[i + 1] === '\n') i += 2
      else i += 1
      // 行数截断保护（含表头共 MAX_CSV_ROWS + 1 行）
      if (rows.length >= MAX_CSV_ROWS + 1) {
        truncated = true
        stopAt = i
        break
      }
      continue
    }
    field += ch
    i += 1
  }

  // 文件末尾无换行时冲刷最后一个字段 / 行
  if (i >= n && (field.length > 0 || row.length > 0)) {
    row.push(field)
    rows.push(row)
  }

  if (inQuotes) {
    issues.push('文件末尾存在未闭合的引号字段，已按原文保留显示。')
  }

  // 估算总行数（截断时基于剩余文本换行数近似）
  let estimatedTotalRows = rows.length
  if (truncated && stopAt >= 0) {
    estimatedTotalRows = rows.length + countNewlines(src, stopAt)
  }

  // 字段数一致性检查（与表头比较，最多提示 3 个行号）
  const header = rows[0] ?? []
  if (header.length > 0) {
    const mismatched: number[] = []
    for (let r = 1; r < rows.length; r++) {
      if (rows[r].length !== header.length && mismatched.length < 3) {
        mismatched.push(r + 1)
      }
    }
    if (mismatched.length > 0) {
      issues.push(`第 ${mismatched.join('、')} 行字段数与表头不一致，已按原样显示。`)
    }
  }

  return {
    header,
    rows: rows.slice(1),
    delimiter,
    truncated,
    estimatedTotalRows,
    issues,
  }
}