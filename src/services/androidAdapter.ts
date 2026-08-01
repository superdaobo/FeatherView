/**
 * Android 适配层（夜间架构合同 §9 / §11 / §14）。
 *
 * `platformService` 为共享文件（禁止修改），Android 专属逻辑集中在本模块：
 * - content:// 检测与 DocumentSource 构造（uriToSource）
 * - read_uri / copy_uri_to_temp / uri_temp_info 命令封装（Rust 命令未注册时降级报错）
 * - 外部打开事件统一入口 listenUriOpen（合同 §11：Rust emit("external-uri-open") +
 *   Kotlin evaluateJavascript 双通道兜底）
 * - 平台能力降级 getAndroidCapabilities（合同 §14，供主 Agent 合入 platformService）
 */
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { DocumentSource } from '../types'
import { getExtension, getFileName } from '../utils'
import { createSourceFromUri, isTauri } from './platformService'

/** 是否 content:// URI（Android SAF） */
export function isContentUri(uri: string): boolean {
  return uri.startsWith('content://')
}

/** 是否 file:// URI（iOS 文件 App / 本地导出预留） */
export function isFileUri(uri: string): boolean {
  return uri.startsWith('file://')
}

/** MIME → 扩展名（与 Kotlin 桥接 extensionFor 保持一致） */
const MIME_EXTENSION: Record<string, string> = {
  'text/markdown': 'md',
  'text/plain': 'txt',
  'application/json': 'json',
  'application/x-yaml': 'yaml',
  'text/yaml': 'yaml',
  'application/toml': 'toml',
  'application/xml': 'xml',
  'text/xml': 'xml',
  'text/html': 'html',
  'text/css': 'css',
  'application/javascript': 'js',
  'application/pdf': 'pdf',
  'image/png': 'png',
  'image/jpeg': 'jpg',
  'image/webp': 'webp',
  'image/gif': 'gif',
  'image/svg+xml': 'svg',
}

/** 从 MIME 推导扩展名（不含点；未知返回 ''） */
export function extensionFromMime(mimeType?: string): string {
  if (!mimeType) return ''
  const base = mimeType.split(';')[0].trim().toLowerCase()
  return MIME_EXTENSION[base] ?? ''
}

/**
 * 从 content:// / file:// URI 构造统一 DocumentSource。
 * content:// 路径末段多为文档 id（无文件名），name/extension 以 mimeType 优先推导。
 */
export function uriToSource(uri: string, mimeType?: string): DocumentSource {
  const extension = extensionFromMime(mimeType) || getExtension(uri)
  const fallback = getFileName(uri)
  // 路径末段形似纯 id（如 msf:1234 / document/42）时生成可读名
  const looksLikeId = /^[a-zA-Z0-9:_%.-]{1,64}$/.test(fallback) && !fallback.includes('.')
  const name = looksLikeId ? `文档.${extension || 'txt'}` : fallback || `文档.${extension || 'txt'}`
  return { ...createSourceFromUri(uri, mimeType), extension, name }
}

/** Rust read_uri 返回结构（camelCase；对齐合同 §2 ReadRangeResult） */
export interface UriReadResult {
  bytesRead: number
  nextOffset: number
  eof: boolean
  text?: string
  encoding?: string
  isBinary: boolean
  size: number
}

/** Rust copy_uri_to_temp / uri_temp_info 返回结构 */
export interface UriCacheInfo {
  cached: boolean
  path?: string
  size?: number
  name?: string
  mimeType?: string
}

/** read_uri 错误码（与 uri.rs UriError::code 对应） */
export const URI_NOT_CACHED = 'URI_NOT_CACHED'
export const URI_BRIDGE_UNAVAILABLE = 'URI_BRIDGE_UNAVAILABLE'
export const URI_UNSUPPORTED = 'URI_UNSUPPORTED'
export const URI_RANGE_ENCODING_UNSUPPORTED = 'URI_RANGE_ENCODING_UNSUPPORTED'

/** 从错误对象提取 code（tauri invoke 错误为 { title, message, code }） */
function errorCode(err: unknown): string {
  if (err && typeof err === 'object') {
    const e = err as { code?: unknown }
    if (typeof e.code === 'string') return e.code
  }
  return 'UNKNOWN'
}

/**
 * 读取 URI 内容（合同 §9 read_uri 命令）。
 * 降级：命令未注册 / 桥接未就绪时抛 `{ title, message, code }` 结构错误，
 * 调用方用 isUriNotCached 等判定并引导导入。
 */
export async function readUri(
  uri: string,
  offset?: number,
  length?: number,
): Promise<UriReadResult> {
  if (!isTauri()) {
    throw {
      title: '当前环境不支持读取',
      message: 'URI 读取需要 Tauri 运行环境。',
      code: 'NO_TAURI_ENV',
    }
  }
  return await invoke<UriReadResult>('read_uri', { uri, offset, length })
}

/** 把 content:// 拷贝到应用缓存（合同 §9；Android 专属，桌面返回 URI_UNSUPPORTED） */
export async function copyUriToTemp(uri: string): Promise<UriCacheInfo> {
  return await invoke<UriCacheInfo>('copy_uri_to_temp', { uri })
}

/** 查询 URI 缓存状态 */
export async function uriTempInfo(uri: string): Promise<UriCacheInfo> {
  return await invoke<UriCacheInfo>('uri_temp_info', { uri })
}

/** 判断错误是否为"未导入缓存"（引导导入流程） */
export function isUriNotCached(err: unknown): boolean {
  return errorCode(err) === URI_NOT_CACHED
}

/** 判断错误是否为"桥接未就绪"（降级提示） */
export function isBridgeUnavailable(err: unknown): boolean {
  return errorCode(err) === URI_BRIDGE_UNAVAILABLE
}

/** 合同 §14 平台能力结构 */
export interface PlatformCapabilities {
  fileManagement: boolean
  folderBrowsing: boolean
  nativePath: boolean
  previewHandler: boolean
}

/** Android 能力降级（合同 §14）：content 模式无 path 类能力 */
export function getAndroidCapabilities(): PlatformCapabilities {
  return {
    fileManagement: false,
    folderBrowsing: false,
    nativePath: false,
    previewHandler: false,
  }
}

/** source 是否为 URI 来源（需走 read_uri 而非 read_file） */
export function sourceNeedsUriRead(source: DocumentSource): boolean {
  return !source.path && !!source.uri && (isContentUri(source.uri) || isFileUri(source.uri))
}

export type UriOpenHandler = (uri: string) => void

/**
 * 外部打开 / 分享 URI 统一入口（合同 §11）。
 * 通道 1：Tauri 事件 external-uri-open（Rust emit；桥接转发合入后生效）。
 * 通道 2：window.__featherviewAndroidUri（Kotlin onNewIntent evaluateJavascript 直投，当前主通道）。
 * 返回取消订阅函数。
 */
export function listenUriOpen(handler: UriOpenHandler): () => void {
  if (typeof window === 'undefined') return () => {}
  const stops: Array<() => void> = []

  if (isTauri()) {
    let stop = () => {}
    void listen<string>('external-uri-open', (event) => {
      handler(event.payload)
    }).then((fn) => {
      stop = fn
    })
    stops.push(() => stop())
  }

  const win = window as unknown as { __featherviewAndroidUri?: (uri: string) => void }
  win.__featherviewAndroidUri = (uri: string) => handler(uri)
  stops.push(() => {
    delete win.__featherviewAndroidUri
  })

  return () => stops.forEach((stop) => stop())
}