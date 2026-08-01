/**
 * 渲染器注册表：新增文件类型只需注册新的 RendererDefinition，
 * ReaderPage 通过 matchRenderer 分发，不关心具体格式。
 */
import type { DocumentMeta, RendererDefinition } from '../types'

const registry = new Map<string, RendererDefinition>()

export function registerRenderer(definition: RendererDefinition): void {
  registry.set(definition.id, definition)
}

export function getRenderer(id: string): RendererDefinition | undefined {
  return registry.get(id)
}

export function listRenderers(): RendererDefinition[] {
  return [...registry.values()]
}

/**
 * 匹配渲染器：优先精确扩展名匹配，其次 canHandle 兜底。
 */
export function matchRenderer(doc: DocumentMeta): RendererDefinition | undefined {
  const ext = doc.source.extension.toLowerCase()
  for (const def of registry.values()) {
    if (def.extensions.includes(ext)) return def
  }
  for (const def of registry.values()) {
    if (def.canHandle(doc)) return def
  }
  return undefined
}

/** 已注册的扩展名集合（用于首页支持格式说明） */
export function supportedExtensions(): string[] {
  const set = new Set<string>()
  for (const def of registry.values()) {
    for (const ext of def.extensions) set.add(ext)
  }
  return [...set]
}
