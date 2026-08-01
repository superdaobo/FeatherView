<script setup lang="ts">
/**
 * Markdown 渲染器。
 * 管线：markdown-it → DOMPurify → 安全 HTML
 * 特性：代码高亮/复制、相对图片解析、外部链接系统浏览器打开、缺失图片占位。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { openUrl } from '@tauri-apps/plugin-opener'
import { invoke } from '@tauri-apps/api/core'
import type { DocumentMeta } from '../../types'
import { getDirName } from '../../utils'
import { isTauri } from '../../services/platformService'
import { extractToc, renderMarkdown } from './markdown'
import { useToast } from '../../composables/useToast'
import { useSettingsStore } from '../../stores/settings'

const props = defineProps<{
  document: DocumentMeta
  content: string
}>()

const emit = defineEmits<{
  toc: [toc: Array<{ level: number; text: string; id: string }>]
}>()

const { show: showToast } = useToast()
const settingsStore = useSettingsStore()
const containerRef = ref<HTMLElement | null>(null)
const renderedHtml = computed(() => renderMarkdown(props.content))

// 当前目录（用于解析相对图片）
const baseDir = computed(() => getDirName(props.document.source.path ?? ''))

// ---------- 标题目录 ----------
watch(
  renderedHtml,
  (html) => {
    emit('toc', extractToc(html))
  },
  { immediate: true },
)

// ---------- 相对图片解析 ----------
async function resolveRelativeImage(img: HTMLImageElement, src: string): Promise<void> {
  if (!baseDir.value) {
    showMissingImage(img, src)
    return
  }
  // 规范化相对路径（处理 ../ 与 ./）
  const segments = [...baseDir.value.split(/[\\/]/), ...src.split('/')]
  const normalized: string[] = []
  for (const seg of segments) {
    if (!seg || seg === '.') continue
    if (seg === '..') {
      normalized.pop()
    } else {
      normalized.push(seg)
    }
  }
  const fullPath = normalized.join('\\')
  try {
    const result = await invoke<{
      content?: string
      bytesBase64?: string
      isBinary: boolean
    }>('read_file', { path: fullPath })
    let dataUrl: string
    if (result.isBinary && result.bytesBase64) {
      dataUrl = `data:image/png;base64,${result.bytesBase64}`
    } else if (result.content !== undefined) {
      // SVG 等文本类图片
      dataUrl = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(result.content)}`
    } else {
      showMissingImage(img, src)
      return
    }
    img.src = dataUrl
  } catch {
    showMissingImage(img, src)
  }
}

function showMissingImage(img: HTMLImageElement, src: string): void {
  const placeholder = document.createElement('span')
  placeholder.className = 'img-missing'
  placeholder.textContent = `图片加载失败：${src}`
  img.replaceWith(placeholder)
}

function processImages(): void {
  if (!containerRef.value) return
  containerRef.value.querySelectorAll<HTMLImageElement>('img').forEach((img) => {
    const src = img.getAttribute('src') ?? ''
    if (/^(https?:|data:|blob:)/i.test(src)) return
    // 已经是 data URL（本组件处理过）跳过；绝对路径直接读取
    img.addEventListener('error', () => showMissingImage(img, src), { once: true })
    if (src.startsWith('data:')) return
    void resolveRelativeImage(img, src)
  })
}

// ---------- 代码块复制按钮 ----------
function attachCopyButtons(): void {
  if (!containerRef.value) return
  containerRef.value.querySelectorAll('pre').forEach((pre) => {
    if (pre.querySelector('.copy-code-btn')) return
    const btn = document.createElement('button')
    btn.className = 'copy-code-btn'
    btn.type = 'button'
    btn.title = '复制代码'
    btn.textContent = '复制'
    btn.addEventListener('click', async (e) => {
      e.stopPropagation()
      const code = pre.querySelector('code')?.textContent ?? ''
      try {
        await navigator.clipboard.writeText(code)
        btn.textContent = '已复制'
        setTimeout(() => (btn.textContent = '复制'), 1500)
      } catch {
        showToast('复制失败', { kind: 'error' })
      }
    })
    pre.appendChild(btn)
  })
}

// ---------- 链接拦截 ----------
function onContainerClick(event: MouseEvent): void {
  const target = event.target as HTMLElement
  const anchor = target.closest('a')
  if (!anchor) return
  const href = anchor.getAttribute('href') ?? ''
  if (/^https?:\/\//i.test(href)) {
    event.preventDefault()
    if (isTauri()) {
      void openUrl(href)
    } else {
      window.open(href, '_blank', 'noopener')
    }
  }
}

let processed = false
onMounted(() => {
  if (processed) return
  processed = true
  processImages()
  attachCopyButtons()
})

watch(renderedHtml, () => {
  // v-html 更新后重新处理图片与复制按钮（下一帧保证 DOM 就绪）
  requestAnimationFrame(() => {
    processImages()
    attachCopyButtons()
  })
})

onBeforeUnmount(() => {
  processed = false
})

defineExpose({ containerRef })
</script>

<template>
  <div
    ref="containerRef"
    class="markdown-body reader-scroll"
    :data-width="settingsStore.settings.contentWidth"
    :data-font="settingsStore.settings.fontFamily"
    data-renderer="markdown"
    @click="onContainerClick"
    v-html="renderedHtml"
  />
</template>
