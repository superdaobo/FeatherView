/**
 * 文档服务：统一文件读取入口。
 * 所有文件读取都经由 Rust read_file 命令，错误统一转换为 AppErrorInfo。
 * 大文本文件（超过 read_file 上限且为已知文本扩展名）返回"骨架"，
 * 由文本渲染器经 open_read_session 会话分块读取（Rust 未就绪时渲染器降级提示）。
 */
import { invoke } from '@tauri-apps/api/core'
import type { AppErrorInfo, DocumentMeta, DocumentSource, ReadFileResult } from '../types'
import { isTextFileExtension } from '../types/nightly'
import { isTauri } from './platformService'

/** 前端兜底的大小限制（与 Rust 侧一致） */
export const MAX_TEXT_BYTES = 10 * 1024 * 1024
export const MAX_BINARY_BYTES = 50 * 1024 * 1024

/** 将 Rust 返回的错误对象规范化为 AppErrorInfo */
export function normalizeError(err: unknown): AppErrorInfo {
  if (err && typeof err === 'object') {
    const e = err as Record<string, unknown>
    if (typeof e.title === 'string' && typeof e.message === 'string') {
      return { title: e.title, message: e.message, code: String(e.code ?? 'UNKNOWN') }
    }
    if (typeof e.message === 'string') {
      return { title: '发生错误', message: e.message, code: String(e.code ?? 'UNKNOWN') }
    }
  }
  const message = err instanceof Error ? err.message : String(err)
  return { title: '发生错误', message, code: 'UNKNOWN' }
}

/** 判断文件是否超过读取上限 */
export function isTooLarge(size: number | undefined, isBinary: boolean): boolean {
  if (!size) return false
  return isBinary ? size > MAX_BINARY_BYTES : size > MAX_TEXT_BYTES
}

/**
 * 打开并读取文档。
 * @returns 文档元信息 + 读取结果
 */
export async function openDocument(source: DocumentSource): Promise<{ meta: DocumentMeta; result: ReadFileResult }> {
  if (!source.path) {
    throw {
      title: '暂不支持该来源',
      message: '当前版本仅支持从本地路径打开文件。移动端 URI 支持将在后续版本提供。',
      code: 'UNSUPPORTED_SOURCE',
    }
  }

  if (!isTauri()) {
    // 浏览器开发模式：无法调用 Rust，返回明确错误
    throw {
      title: '当前环境不支持读取',
      message: '文件读取需要 Tauri 运行环境。请使用 pnpm tauri dev 启动应用。',
      code: 'NO_TAURI_ENV',
    }
  }

  // 大文本文件（超过 read_file 上限）：返回"骨架"（content 为空 + size），
  // 由文本渲染器经 open_read_session / read_range 会话分块读取；
  // Rust 会话命令未就绪时渲染器显示明确降级提示（不白屏、不抛错）
  if (isTextFileExtension(source.extension)) {
    const fileMeta = await readFileMetadata(source.path)
    if (fileMeta?.size && fileMeta.size > MAX_TEXT_BYTES) {
      const meta: DocumentMeta = {
        source,
        size: fileMeta.size,
        modifiedAt: fileMeta.modifiedAt,
        isBinary: false,
      }
      const result: ReadFileResult = {
        content: '',
        size: fileMeta.size,
        modifiedAt: fileMeta.modifiedAt,
        isBinary: false,
      }
      return { meta, result }
    }
  }

  try {
    const result = await invoke<ReadFileResult>('read_file', { path: source.path })
    const meta: DocumentMeta = {
      source,
      size: result.size,
      modifiedAt: result.modifiedAt,
      encoding: result.encoding,
      isBinary: result.isBinary,
    }
    return { meta, result }
  } catch (err) {
    const info = normalizeError(err)
    // 保持调用链可读
    const wrapped = new Error(info.message) as Error & { info?: AppErrorInfo }
    wrapped.info = info
    throw wrapped
  }
}

/** 检查文件是否存在 */
export async function checkFileExists(path: string): Promise<boolean> {
  if (!isTauri()) return true
  try {
    return await invoke<boolean>('file_exists', { path })
  } catch {
    return false
  }
}

/** 读取文件元信息 */
export async function readFileMetadata(path: string): Promise<{ size?: number; modifiedAt?: number } | null> {
  if (!isTauri()) return null
  try {
    const meta = await invoke<{ size: number; modifiedAt?: number; isFile: boolean }>('file_metadata', { path })
    return { size: meta.size, modifiedAt: meta.modifiedAt }
  } catch {
    return null
  }
}

/** 从错误对象中提取 AppErrorInfo（若存在） */
export function getErrorInfo(err: unknown): AppErrorInfo {
  if (err instanceof Error && 'info' in err && err.info) {
    return (err as Error & { info: AppErrorInfo }).info
  }
  return normalizeError(err)
}