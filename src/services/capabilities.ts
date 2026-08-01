/**
 * 平台能力（临时实现；集成时主 Agent 合入 platformService 第 14 节接口）。
 * 本文件不是共享文件，可被整体替换。
 */
import { invoke } from '@tauri-apps/api/core'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import type { FolderEntry, PlatformCapabilities } from '../types/nightly'
import { isTauri } from './platformService'

/** 当前平台能力（桌面端全开；移动端由集成版 platformService 覆盖） */
export function getPlatformCapabilities(): PlatformCapabilities {
  if (!isTauri()) {
    // 浏览器开发模式：按桌面能力展示（便于前端调试，不做移动端降级模拟）
    return { fileManagement: true, folderBrowsing: true, nativePath: true, previewHandler: false }
  }
  return { fileManagement: true, folderBrowsing: true, nativePath: true, previewHandler: true }
}

/** 系统文件夹选择对话框；取消 / 不支持返回 null */
export async function pickFolder(): Promise<string | null> {
  if (!isTauri()) return null
  try {
    const selected = await dialogOpen({ multiple: false, directory: true, title: '选择文件夹' })
    return typeof selected === 'string' ? selected : null
  } catch {
    return null
  }
}

/**
 * 列出目录内容（Rust list_dir 命令就绪后启用）。
 * 命令未实现 / 失败返回 null，调用方（首页）应降级提示而非报错。
 */
export async function listDirectory(dir: string): Promise<FolderEntry[] | null> {
  if (!isTauri()) return null
  try {
    const entries = await invoke<FolderEntry[]>('list_dir', { path: dir })
    return Array.isArray(entries) ? entries : null
  } catch {
    return null
  }
}