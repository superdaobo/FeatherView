/**
 * 大文件范围读取服务（合同第 2/4 节）。
 *
 * 封装 Rust 命令 open_read_session / read_range / close_read_session。
 * Rust 未实现（命令不存在）时，openReadSession 返回 null，readRange 返回 null，
 * 由调用方（TextViewer 会话读取 / documentService 骨架流程）优雅降级，
 * 不抛出未捕获异常、不影响主流程。
 */
import { invoke } from '@tauri-apps/api/core'
import type { OpenReadSessionResult, ReadRangeResult } from '../types/nightly'

/** 单次 read_range 长度上限（合同：1MB） */
export const MAX_SESSION_CHUNK_BYTES = 1024 * 1024
/** 默认分块大小（合同：256KB） */
export const DEFAULT_SESSION_CHUNK_BYTES = 256 * 1024
/** 前端防护：单会话累计读取上限（200MB），防止异常循环耗尽内存 */
export const MAX_SESSION_TOTAL_BYTES = 200 * 1024 * 1024

/** 打开范围读取会话；命令未实现 / 打开失败返回 null（调用方降级） */
export async function openReadSession(path: string): Promise<OpenReadSessionResult | null> {
  try {
    const res = await invoke<OpenReadSessionResult>('open_read_session', { path })
    if (!res || typeof res.sessionId !== 'string' || !res.info) return null
    return res
  } catch {
    return null
  }
}

/** 读取指定字节范围；失败返回 null */
export async function readRange(
  sessionId: string,
  offset: number,
  length: number,
): Promise<ReadRangeResult | null> {
  try {
    return await invoke<ReadRangeResult>('read_range', { sessionId, offset, length })
  } catch {
    return null
  }
}

/** 关闭会话（幂等；失败静默——Rust 侧 session map 上限淘汰兜底） */
export async function closeReadSession(sessionId: string): Promise<void> {
  try {
    await invoke('close_read_session', { sessionId })
  } catch {
    // 关闭失败不阻断主流程
  }
}

/**
 * 顺序读完整会话内容（分块循环，按 nextOffset 推进直到 eof）。
 * @throws 读取失败或累计超过 MAX_SESSION_TOTAL_BYTES 时抛出 Error
 */
export async function readSessionAll(
  sessionId: string,
  chunkBytes = DEFAULT_SESSION_CHUNK_BYTES,
): Promise<string> {
  const chunk = Math.min(Math.max(1, Math.floor(chunkBytes)), MAX_SESSION_CHUNK_BYTES)
  const parts: string[] = []
  let offset = 0
  let totalChars = 0
  for (;;) {
    const res = await readRange(sessionId, offset, chunk)
    if (!res) throw new Error('读取大文件会话失败，请重试。')
    if (res.text) {
      parts.push(res.text)
      totalChars += res.text.length
    }
    if (totalChars > MAX_SESSION_TOTAL_BYTES) {
      throw new Error('文件过大，超出前端读取上限。')
    }
    offset = res.nextOffset
    if (res.eof) break
  }
  return parts.join('')
}