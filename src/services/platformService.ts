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
import type { DocumentSource } from '../types'
import { createSourceId, getExtension, getFileName } from '../utils'

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
  source: DocumentSource | null,
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
        const path = e.paths[0]
        if (path) {
          handler(createSourceFromPath(path), 'drop')
        } else {
          handler(null, 'drop')
        }
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
 * 启动参数 / 文件关联打开入口（预留统一处理）。
 * 桌面端通过 Rust command 获取进程参数，过滤出第一个存在的文件路径。
 * 移动端后续在此处理分享/打开 intent 的 content:// 与 file:// URI。
 */
export async function resolveInitialDocument(): Promise<DocumentSource | null> {
  if (!isTauri()) return null
  try {
    const args = await invoke<string[]>('startup_args')
    for (const arg of args.slice(1)) {
      if (arg && !arg.startsWith('-')) {
        const exists = await invoke<boolean>('file_exists', { path: arg })
        if (exists) return createSourceFromPath(arg)
      }
    }
  } catch {
    // 忽略：无启动参数时静默返回 null
  }
  return null
}
