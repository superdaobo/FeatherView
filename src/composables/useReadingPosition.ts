/**
 * 阅读位置跟踪 composable：
 * - 滚动保存（500ms 防抖，不每次滚动写 localStorage）
 * - 恢复（渲染完成后有限重试，headingId → scrollRatio → scrollTop → 顶部）
 * - 保存时机：滚动防抖 / 打开新文件前 / 组件卸载前 / 窗口关闭（pagehide）
 */
import { onBeforeUnmount, onMounted, ref, watch, type Ref } from 'vue'
import type { DocumentMeta, DocumentSource } from '../types'
import { getDocumentIdentity } from '../services/platformService'
import {
  clampScrollTop,
  getPosition,
  savePosition,
} from '../services/readingPositionService'

const SAVE_DEBOUNCE_MS = 500
const MAX_RESTORE_ATTEMPTS = 5
/** 内容高度变化超过该比例时放弃绝对位置、改用比例 */
const HEIGHT_TOLERANCE = 0.2

export function useReadingPosition(
  container: Ref<HTMLElement | null>,
  source: Ref<DocumentSource | null>,
  meta: Ref<DocumentMeta | null>,
  rendererId: Ref<string | undefined>,
  enabled: Ref<boolean>,
  onRestored?: (record: { headingId?: string }) => void,
) {
  let debounceTimer: number | undefined
  /** 当前已恢复位置的 identity（避免重复恢复/重复 Toast） */
  const restoredIdentity = ref<string | null>(null)

  function currentIdentity(): string | null {
    return meta.value ? getDocumentIdentity(meta.value.source) : null
  }

  function flushSave(): void {
    const el = container.value
    const id = currentIdentity()
    if (!el || !id || !enabled.value || !meta.value) return
    const scrollTop = el.scrollTop
    const contentHeight = el.scrollHeight
    const viewportHeight = el.clientHeight
    const max = contentHeight - viewportHeight
    const scrollRatio = max > 0 ? Math.min(1, Math.max(0, scrollTop / max)) : 0
    // Markdown：视口上方最近的标题锚点
    let headingId: string | undefined
    if (rendererId.value === 'markdown') {
      const headings = el.querySelectorAll<HTMLElement>('h1, h2, h3')
      for (const heading of headings) {
        if (heading.offsetTop <= scrollTop + viewportHeight * 0.3) {
          headingId = heading.id || undefined
        } else {
          break
        }
      }
    }
    savePosition({
      sourceId: id,
      path: meta.value.source.path,
      uri: meta.value.source.uri,
      rendererId: rendererId.value ?? 'unknown',
      scrollTop,
      scrollRatio,
      viewportHeight,
      contentHeight,
      headingId,
      updatedAt: Date.now(),
    })
  }

  function scheduleSave(): void {
    if (!enabled.value) return
    window.clearTimeout(debounceTimer)
    debounceTimer = window.setTimeout(flushSave, SAVE_DEBOUNCE_MS)
  }

  function applyRestore(): void {
    const el = container.value
    const id = currentIdentity()
    if (!el || !id || !enabled.value) return
    const record = getPosition(id)
    if (!record) {
      restoredIdentity.value = id
      el.scrollTop = 0
      return
    }

    // 1. Markdown 标题锚点优先
    if (record.headingId) {
      const target = el.querySelector(`#${CSS.escape(record.headingId)}`)
      if (target instanceof HTMLElement) {
        target.scrollIntoView({ block: 'start' })
        restoredIdentity.value = id
        onRestored?.(record)
        return
      }
    }

    // 2. 内容高度接近时用绝对位置
    const max = el.scrollHeight - el.clientHeight
    if (max > 0) {
      const heightDiff =
        record.contentHeight > 0
          ? Math.abs(record.contentHeight - el.scrollHeight) / record.contentHeight
          : 1
      if (heightDiff <= HEIGHT_TOLERANCE) {
        el.scrollTop = clampScrollTop(record.scrollTop, max)
      } else {
        // 3. 高度变化：用相对比例
        el.scrollTop = clampScrollTop(record.scrollRatio * max, max)
      }
      restoredIdentity.value = id
      onRestored?.(record)
    }
  }

  function tryRestore(attempt = 0): void {
    const el = container.value
    const id = currentIdentity()
    if (!el || !id || !enabled.value) return
    if (restoredIdentity.value === id) return
    if (attempt >= MAX_RESTORE_ATTEMPTS) return
    if (el.scrollHeight > 0) {
      applyRestore()
      return
    }
    requestAnimationFrame(() => tryRestore(attempt + 1))
  }

  // 容器或文档变化时尝试恢复
  watch([container, meta], () => {
    requestAnimationFrame(() => tryRestore(0))
  })

  // 容器或文档变化时：绑定滚动监听并尝试恢复（容器可能晚于组件挂载就绪）
  let scrollBound = false
  function onScroll(): void {
    scheduleSave()
  }

  watch(container, (el, old) => {
    if (scrollBound && old) {
      old.removeEventListener('scroll', onScroll)
    }
    scrollBound = false
    if (el) {
      el.addEventListener('scroll', onScroll, { passive: true })
      scrollBound = true
    }
    requestAnimationFrame(() => tryRestore(0))
  })

  // 打开新文件前（source 变化先于渲染器卸载）保存旧位置；
  // 引用比较：同一文件重复打开也会触发保存
  watch(
    () => source.value,
    (newSource, oldSource) => {
      if (newSource !== oldSource && oldSource !== null) {
        flushSave()
      }
    },
  )

  onMounted(() => {
    // 窗口关闭/刷新前最终保存
    window.addEventListener('pagehide', flushSave)
  })

  onBeforeUnmount(() => {
    flushSave()
    window.removeEventListener('pagehide', flushSave)
    if (scrollBound && container.value) {
      container.value.removeEventListener('scroll', onScroll)
    }
    window.clearTimeout(debounceTimer)
  })

  /** 手动立即保存（文件切换前由外部调用） */
  function saveNow(): void {
    flushSave()
  }

  return { flushSave, saveNow, restoredIdentity }
}
