/**
 * Nightly 新增类型（不修改共享的 types/index.ts）。
 *
 * 对应《夜间架构合同》：
 * - 第 2/3 节：大文件范围读取协议（LargeFileInfo / ReadRangeResult / OpenReadSessionResult）
 * - 第 6 节：标签页状态结构（TabState）
 * - 第 14 节：平台功能检测接口（PlatformCapabilities）
 * 集成时由主 Agent 决定是否并入 types/index.ts。
 */
import type { DocumentMeta, DocumentSource, ReadingPositionRecord } from './index'

/** 大文件元信息（合同第 3 节，open_read_session 返回） */
export interface LargeFileInfo {
  size: number
  mtime: number
  encoding: string
  isText: boolean
  /** 建议单次读取块大小（字节），默认 256KB */
  suggestedChunkBytes: number
}

/** open_read_session 命令返回 */
export interface OpenReadSessionResult {
  sessionId: string
  info: LargeFileInfo
}

/** read_range 命令返回（合同第 2 节） */
export interface ReadRangeResult {
  bytesRead: number
  nextOffset: number
  eof: boolean
  text: string
  encoding: string
  /** 会话打开时的文件 mtime（用于变化检测） */
  sessionModifiedAt?: number
}

/** 标签页状态（合同第 6 节） */
export interface TabState {
  tabId: string
  source: DocumentSource
  meta?: DocumentMeta
  rendererId?: string
  /** 复用 v0.1.1 阅读位置记录（标签切换恢复用） */
  position?: ReadingPositionRecord
  pinned?: boolean
}

/** 平台能力（合同第 14 节；集成时合入 platformService） */
export interface PlatformCapabilities {
  /** 复制/移动/重命名/删除（移动端 content 模式为 false） */
  fileManagement: boolean
  /** 文件夹浏览（桌面 true） */
  folderBrowsing: boolean
  /** source.path 可用 */
  nativePath: boolean
  /** Windows 桌面 true（仅信息用途） */
  previewHandler: boolean
}

/** 收藏记录（P2） */
export interface FavoriteItem {
  path: string
  name: string
  extension: string
  size?: number
  addedAt: number
}

/** 文件夹条目（capabilities.listDirectory 返回；Rust list_dir 就绪后启用） */
export interface FolderEntry {
  name: string
  path: string
  isDir: boolean
  size?: number
}

/** 已知文本类扩展名（大文件会话读取判定用；与 platformService.SUPPORTED_EXTENSIONS 的文本子集一致） */
export const TEXT_FILE_EXTENSIONS = [
  // Markdown / 纯文本
  'md', 'markdown', 'mdown', 'txt', 'log',
  // 代码
  'js', 'ts', 'jsx', 'tsx', 'vue', 'css', 'html', 'htm',
  'py', 'rs', 'java', 'c', 'cpp', 'h', 'hpp', 'cs', 'go',
  'sh', 'ps1', 'sql', 'bat', 'cmd',
  // 配置与结构化数据
  'json', 'yaml', 'yml', 'toml', 'xml', 'ini', 'env',
] as const

/** 扩展名是否为已知文本类型（大文件骨架判定用） */
export function isTextFileExtension(extension: string): boolean {
  return (TEXT_FILE_EXTENSIONS as readonly string[]).includes(extension.toLowerCase())
}