<script setup lang="ts">
/**
 * 代码渲染器（只读语法高亮）。
 * 支持：20+ 语言按需注册的高亮、行号、自动换行、复制全文、大文件截断保护。
 */
import { computed, ref, watch } from 'vue'
import type { DocumentMeta } from '../../types'
import { useSettingsStore } from '../../stores/settings'
import { hljsInstance } from '../markdown/markdown'
import { useToast } from '../../composables/useToast'

const props = defineProps<{
  document: DocumentMeta
  content: string
}>()

const settingsStore = useSettingsStore()
const { show: showToast } = useToast()
const containerRef = ref<HTMLElement | null>(null)

const MAX_PREVIEW_CHARS = 2_000_000
const MAX_LINE_MODE_CHARS = 500_000

const truncated = computed(() => props.content.length > MAX_PREVIEW_CHARS)
const previewContent = computed(() =>
  truncated.value ? props.content.slice(0, MAX_PREVIEW_CHARS) : props.content,
)

const language = computed(() => {
  const ext = props.document.source.extension
  if (!ext) return ''
  // highlight.js 语言名与扩展名映射
  const map: Record<string, string> = {
    js: 'javascript',
    jsx: 'javascript',
    mjs: 'javascript',
    ts: 'typescript',
    tsx: 'typescript',
    vue: 'xml',
    html: 'xml',
    htm: 'xml',
    xml: 'xml',
    svg: 'xml',
    py: 'python',
    rs: 'rust',
    java: 'java',
    c: 'c',
    h: 'c',
    cpp: 'cpp',
    hpp: 'cpp',
    cc: 'cpp',
    cs: 'csharp',
    go: 'go',
    sh: 'bash',
    bash: 'bash',
    ps1: 'powershell',
    yaml: 'yaml',
    yml: 'yaml',
    toml: 'ini',
    ini: 'ini',
    env: 'ini',
    sql: 'sql',
    md: 'markdown',
    markdown: 'markdown',
    css: 'css',
    json: 'json',
  }
  return map[ext] ?? ''
})

const highlightedHtml = computed(() => {
  const code = previewContent.value
  const lang = language.value
  if (lang && hljsInstance.getLanguage(lang)) {
    try {
      return hljsInstance.highlight(code, { language: lang, ignoreIllegals: true }).value
    } catch {
      // 回退转义
    }
  }
  return hljsInstance.highlightAuto(code).value
})

const useLineMode = computed(() => previewContent.value.length <= MAX_LINE_MODE_CHARS)
const showLineNumbers = computed(
  () => settingsStore.settings.showLineNumbers && useLineMode.value,
)

/** 将高亮 HTML 拆分为带行号的代码行 */
const lineHtml = computed(() => {
  if (!useLineMode.value) return ''
  return highlightedHtml.value
    .split('\n')
    .map(
      (line, i) =>
        `<span class="code-line" data-line="${i + 1}">${showLineNumbers.value ? `<span class="line-num">${i + 1}</span>` : ''}<span class="line-code">${line || ' '}</span></span>`,
    )
    .join('')
})

async function copyAll(): Promise<void> {
  try {
    await navigator.clipboard.writeText(props.content)
    showToast('已复制全文', { kind: 'success' })
  } catch {
    showToast('复制失败', { kind: 'error' })
  }
}

watch(
  () => props.content,
  () => {
    if (containerRef.value) containerRef.value.scrollTop = 0
  },
)

defineExpose({ containerRef })
</script>

<template>
  <div class="code-viewer">
    <div class="code-toolbar">
      <span class="code-lang">{{ language || 'plaintext' }}</span>
      <span
        v-if="truncated"
        class="truncate-hint"
      >大文件已截断预览</span>
      <button
        class="btn btn-small"
        type="button"
        @click="copyAll"
      >
        复制全文
      </button>
    </div>
    <div
      ref="containerRef"
      class="code-content reader-scroll"
    >
      <pre
        class="hljs code-pre"
        :class="{ 'wrap-on': settingsStore.settings.wordWrap, mono: settingsStore.settings.fontFamily === 'monospace' }"
        v-html="useLineMode ? lineHtml : highlightedHtml"
      />
    </div>
  </div>
</template>
