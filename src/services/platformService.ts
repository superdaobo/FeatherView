/**
 * 平台服务：统一文件入口抽象。
 *
 * 所有与平台相关的文件获取方式（按钮选择、拖拽、启动参数、移动端 URI）
 * 都集中在本模块，Vue 组件不得直接处理平台差异。
 */
import { invoke } from '@tauri-apps/api/core'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import type { DragDropEvent } from '@tauri-apps/api/webview'
import type { Event } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'
import type { DocumentSource } from '../types'
import { createSourceId, getExtension, getFileName } from '../utils'
import { parseLaunchArgs } from './launchArgs'

export const SUPPORTED_EXTENSIONS = [
  // Markdown
  'md', 'markdown', 'mdown',
  // 纯文本
  'txt', 'log',
  // 代码与配置
  'js', 'ts', 'jsx', 'tsx', 'vue', 'css', 'html', 'htm', 'py', 'rs', 'java', 'c', 'cpp', 'h',
  'hpp', 'cs', 'go', 'sh', 'ps1', 'json', 'yaml', 'yml', 'toml', 'xml', 'ini', 'env',
  // 图片
  'png', 'jpg', 'jpeg', 'webp', 'gif', 'svg', 'bmp', 'ico', 'avif',
] as const

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

/** 从路径构造统一 DocumentSource（桌面端） */
export function createSourceFromPath(path: string, size?: number): DocumentSource {
  const extension = getExtension(path)
  return {
    id: createSourceId(path),
    name: getFileName(path),
    extension,
    path,
    size,
  }
}

/** 从 URI 构造统一 DocumentSource（移动端 content:// / file:// 预留） */
export function createSourceFromUri(uri: string, mimeType?: string): DocumentSource {
  return {
    id: createSourceId(undefined, uri),
    name: getFileName(uri),
    extension: getExtension(uri),
    uri,
    mimeType,
  }
}

/**
 * 通过系统文件选择对话框选择文件。
 * @returns 用户选择的 DocumentSource；取消时返回 null
 */
export async function pickFile(): Promise<DocumentSource | null> {
  if (!isTauri()) {
    // 浏览器开发模式：提供兜底实现（仅用于前端调试，不用于生产）
    return null
  }
  const selected = await dialogOpen({
    multiple: false,
    directory: false,
    title: '选择要阅读的文件',
  })
  if (!selected || typeof selected !== 'string') return null
  return createSourceFromPath(selected)
}

export type DragDropHandler = (
  sources: DocumentSource[] | null,
  kind: 'enter' | 'over' | 'drop' | 'leave',
) => void

/**
 * 注册窗口拖拽事件（Windows 拖入文件）。
 * 返回取消订阅函数。
 */
export function onFileDragDrop(handler: DragDropHandler): () => void {
  if (!isTauri()) return () => {}

  let unlisten: (() => void) | undefined
  getCurrentWebview()
    .onDragDropEvent((event: Event<DragDropEvent>) => {
      const e = event.payload
      if (e.type === 'enter' || e.type === 'over') {
        handler(null, e.type)
        return
      }
      if (e.type === 'drop') {
        handler(e.paths.map((p) => createSourceFromPath(p)), 'drop')
        return
      }
      // leave
      handler(null, 'leave')
    })
    .then((fn) => {
      unlisten = fn
    })
  return () => unlisten?.()
}

/**
 * 启动参数 / 文件关联打开入口（冷启动）。
 * 通过 Rust startup_args 获取进程参数，解析出第一个有效文件。
 */
export async function resolveInitialDocument(): Promise<DocumentSource | null> {
  if (!isTauri()) return null
  try {
    const args = await invoke<string[]>('startup_args')
    const { files } = await parseLaunchArgs(args, pathExists)
    if (files.length > 0) return createSourceFromPath(files[0])
  } catch {
    // 无启动参数或解析异常时静默返回 null
  }
  return null
}

async function pathExists(path: string): Promise<boolean> {
  try {
    return await invoke<boolean>('file_exists', { path })
  } catch {
    return false
  }
}

/** 检查路径是否为文件（排除文件夹） */
export async function isFilePath(path: string): Promise<boolean> {
  if (!isTauri()) return true
  try {
    const meta = await invoke<{ size: number; modifiedAt?: number; isFile: boolean }>('file_metadata', {
      path,
    })
    return meta.isFile
  } catch {
    return false
  }
}

/**
 * 外部文件打开事件监听器（第二实例参数）。
 * 返回取消订阅函数。
 */
export async function initializeExternalOpenListener(
  handler: (sources: DocumentSource[]) => void,
): Promise<() => void> {
  if (!isTauri()) return () => {}
  const unlisten = await listen<string[]>('external-file-open', (event: Event<string[]>) => {
    void (async () => {
      try {
        const { files } = await parseLaunchArgs(event.payload, pathExists)
        if (files.length > 0) {
          handler(files.map((p) => createSourceFromPath(p)))
        }
      } catch {
        // 参数异常静默，不允许影响应用
      }
    })()
  })
  return unlisten
}

/**
 * 文档唯一标识：Windows 路径规范化（统一分隔符、去尾分隔符、小写归一）。
 * 移动端 URI 走同一接口。
 */
export function getDocumentIdentity(source: DocumentSource): string {
  const raw = source.path ?? source.uri ?? ''
  if (!raw) return `uri:${source.id}`
  if (source.path) {
    // 统一为反斜杠，去除末尾分隔符（保留盘符根如 C:\），小写归一（Windows 大小写不敏感）
    let normalized = raw.replace(/\//g, '\\')
    while (normalized.length > 3 && normalized.endsWith('\\')) {
      normalized = normalized.slice(0, -1)
    }
    return `path:${normalized.toLowerCase()}`
  }
  return `uri:${raw}`
}
