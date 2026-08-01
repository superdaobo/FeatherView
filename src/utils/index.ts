/** 人类可读的文件大小 */
export function formatFileSize(size?: number): string {
  if (size === undefined || size === null || Number.isNaN(size)) return '—'
  if (size < 1024) return `${size} B`
  const kb = size / 1024
  if (kb < 1024) return `${kb.toFixed(1)} KB`
  const mb = kb / 1024
  if (mb < 1024) return `${mb.toFixed(1)} MB`
  return `${(mb / 1024).toFixed(2)} GB`
}

/** 从路径中提取扩展名（小写、不含点） */
export function getExtension(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? path
  const dot = name.lastIndexOf('.')
  if (dot <= 0 || dot === name.length - 1) return ''
  return name.slice(dot + 1).toLowerCase()
}

/** 从路径中提取文件名 */
export function getFileName(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? path
  return name || path
}

/** 从路径中提取所在目录 */
export function getDirName(path: string): string {
  const idx = Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/'))
  return idx > 0 ? path.slice(0, idx) : ''
}

/** 生成文档源 id */
export function createSourceId(path?: string, uri?: string): string {
  return path ?? uri ?? `doc-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
}

/** 统计文本行数 */
export function countLines(text: string): number {
  if (!text) return 0
  return text.split(/\r\n|\r|\n/).length
}

/** 粗略统计字数（中英文混合） */
export function countWords(text: string): number {
  if (!text) return 0
  const cjk = text.match(/[\u4e00-\u9fff\u3400-\u4dbf\uf900-\ufaff]/g)?.length ?? 0
  const words = text
    .replace(/[\u4e00-\u9fff\u3400-\u4dbf\uf900-\ufaff]/g, ' ')
    .split(/\s+/)
    .filter(Boolean).length
  return cjk + words
}

/** 将秒/毫秒格式化为日期字符串 */
export function formatDate(ts?: number): string {
  if (!ts) return '—'
  const d = new Date(ts)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}
