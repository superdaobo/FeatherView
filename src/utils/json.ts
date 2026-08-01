/** JSON 解析与格式化工具 */

export interface JsonParseOk {
  ok: true
  value: unknown
  formatted: string
}

export interface JsonParseFail {
  ok: false
  message: string
}

export type JsonParseResult = JsonParseOk | JsonParseFail

/** 解析 JSON，成功返回格式化文本，失败返回错误信息 */
export function parseJson(content: string): JsonParseResult {
  const raw = content.trim()
  if (!raw) return { ok: true, value: null, formatted: '' }
  try {
    const value = JSON.parse(content)
    return { ok: true, value, formatted: JSON.stringify(value, null, 2) }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err)
    return { ok: false, message }
  }
}
