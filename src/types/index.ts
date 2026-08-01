import type { Component } from 'vue'

/** 最近打开的文件记录 */
export interface RecentFile {
  path: string
  name: string
  extension: string
  lastOpenedAt: number
  size?: number
}

/** 阅读位置记录（v0.1.1） */
export interface ReadingPositionRecord {
  /** 规范化后的文件唯一标识（Windows 路径小写归一） */
  sourceId: string
  path?: string
  uri?: string
  rendererId: string
  /** 绝对滚动位置（px） */
  scrollTop: number
  /** 相对滚动比例（0-1，主恢复依据） */
  scrollRatio: number
  viewportHeight: number
  contentHeight: number
  /** Markdown 最近标题锚点（可选） */
  headingId?: string
  updatedAt: number
}

/**
 * 统一的文件入口抽象。
 * Windows 使用 path；Android/iOS 移动端使用 uri（content:// 或 file://）。
 * 所有平台差异集中在 platformService，组件层不直接接触路径。
 */
export interface DocumentSource {
  id: string
  name: string
  extension: string
  path?: string
  uri?: string
  size?: number
  mimeType?: string
}

/** 当前文档的元信息 */
export interface DocumentMeta {
  source: DocumentSource
  size: number
  modifiedAt?: number
  encoding?: string
  isBinary: boolean
}

/** Rust read_file 命令的返回结构 */
export interface ReadFileResult {
  content?: string
  bytesBase64?: string
  encoding?: string
  size: number
  modifiedAt?: number
  isBinary: boolean
}

/** 统一错误结构（Rust 侧序列化而来，字段为 title/message/code） */
export interface AppErrorInfo {
  title: string
  message: string
  code: string
}

/** 阅读设置，持久化到 localStorage */
export interface ReaderSettings {
  theme: 'system' | 'light' | 'dark'
  fontSize: number
  lineHeight: number
  contentWidth: 'narrow' | 'medium' | 'wide' | 'full'
  wordWrap: boolean
  showLineNumbers: boolean
  fontFamily: 'system' | 'serif' | 'monospace'
  /** 记住并恢复阅读位置 */
  rememberReadingPosition: boolean
}

/** 渲染器统一接口：新增文件类型只需实现该定义并注册 */
export interface RendererDefinition {
  id: string
  name: string
  extensions: string[]
  canHandle(document: DocumentMeta): boolean
  /** 渲染器组件（动态 import 懒加载） */
  component: Component
}

/** 文档加载状态机 */
export type DocumentStatus = 'idle' | 'loading' | 'ready' | 'error'
