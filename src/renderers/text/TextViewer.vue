<script setup lang="ts">
/**
 * 纯文本渲染器（txt / log）。
 * 支持：行号、自动换行、等宽字体、大文本截断保护、大文件会话读取与虚拟滚动。
 *
 * 渲染模式：
 * - 虚拟滚动：行数 > 50000 且未开启自动换行且搜索未激活时启用
 *   （固定行高 24px + 可视区渲染 + padding 占位，保留行号与绝对行号）
 * - 换行降级：自动换行 + 超阈值 → 单文本节点 <pre>（行高不定无法虚拟化）
 * - 搜索降级：searchActive 时临时完整行渲染，保证搜索/高亮/跳转可用
 * - 会话读取：content 为空且 size > 10MB（documentService 骨架）→ 尝试
 *   open_read_session 分块读取；Rust 未实现时显示明确降级提示（不白屏）
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { DocumentMeta } from '../../types'
import { useSettingsStore } from '../../stores/settings'
import { MAX_TEXT_BYTES } from '../../services/documentService'
import { closeReadSession, openReadSession, readSessionAll } from '../../services/largeFileService'
import { countLines, countWords } from '../../utils'

const props = defineProps<{
  document: DocumentMeta
  content: string
  /** 搜索激活时禁用虚拟滚动（ReaderPage 仅对 text 渲染器传入） */
  searchActive?: boolean
}>()

const emit = defineEmits<{
  (e: 'stats', value: { lines: number; words: number }): void
}>()

const settingsStore = useSettingsStore()
const containerRef = ref<HTMLElement | null>(null)

/** 大文本截断保护阈值（字符数） */
const MAX_PREVIEW_CHARS = 2_000_000
/** 行渲染模式阈值：超过后不逐行渲染（避免 DOM 节点爆炸） */
const MAX_LINE_MODE_CHARS = 300_000
/** 虚拟滚动行数阈值 */
const VIRTUAL_THRESHOLD_LINES = 50_000
/** 虚拟滚动固定行高（px）：对应 .line-text 14px × line-height 1.7 ≈ 23.8px */
const VIRTUAL_LINE_HEIGHT = 24
/** 可视区上下额外渲染行数 */
const VIRTUAL_OVERSCAN = 10

// ---------- 大文件会话（合同第 2/4 节；Rust 未就绪时优雅降级） ----------
const sessionText = ref<string | null>(null)
const sessionError = ref<string | null>(null)
const sessionLoading = ref(false)
let sessionId: string | null = null

/** 大文件骨架：documentService 未读到内容（content 为空）且文件超过 read_file 上限 */
const needsSession = computed(
  () => props.content === '' && props.document.size > MAX_TEXT_BYTES && !!props.document.source.path,
)

async function loadSession(): Promise<void> {
  if (!props.document.source.path || sessionId) return
  sessionLoading.value = true
  sessionError.value = null
  const session = await openReadSession(props.document.source.path)
  if (!session) {
    sessionError.value = '当前构建暂不支持大文件分块读取，无法打开超过 10MB 的文本文件。'
    sessionLoading.value = false
    return
  }
  sessionId = session.sessionId
  try {
    if (!session.info.isText) {
      sessionError.value = '该文件不是文本文件，无法以文本方式阅读。'
      return
    }
    sessionText.value = await readSessionAll(sessionId, session.info.suggestedChunkBytes)
  } catch (err) {
    sessionError.value = err instanceof Error ? err.message : '读取大文件失败。'
  } finally {
    sessionLoading.value = false
  }
}

onMounted(() => {
  if (needsSession.value) void loadSession()
})

onBeforeUnmount(() => {
  // 合同第 4 节：组件卸载必须关闭会话，禁止泄漏
  if (sessionId) {
    void closeReadSession(sessionId)
    sessionId = null
  }
})

// ---------- 内容与渲染模式 ----------
const effectiveContent = computed(() => sessionText.value ?? props.content)

const truncated = computed(() => effectiveContent.value.length > MAX_PREVIEW_CHARS)
const previewContent = computed(() =>
  truncated.value ? effectiveContent.value.slice(0, MAX_PREVIEW_CHARS) : effectiveContent.value,
)

const totalLineCount = computed(() => previewContent.value.split(/\r\n|\r|\n/).length)

const lines = computed(() => {
  if (!useLineMode.value) return []
  return previewContent.value.split(/\r\n|\r|\n/)
})

const showLineNumbers = computed(() => {
  return settingsStore.settings.showLineNumbers && useLineMode.value
})

/** 虚拟滚动：固定行高模式（非自动换行 + 行数超阈值 + 搜索未激活） */
const virtualEnabled = computed(
  () =>
    totalLineCount.value > VIRTUAL_THRESHOLD_LINES &&
    !settingsStore.settings.wordWrap &&
    !props.searchActive,
)

/** 行渲染模式：字符数未超阈值，或虚拟滚动/搜索激活时需要逐行渲染 */
const useLineMode = computed(
  () =>
    previewContent.value.length <= MAX_LINE_MODE_CHARS ||
    virtualEnabled.value ||
    props.searchActive === true,
)

/** 单文本节点模式：超出行渲染能力（字符超限，或自动换行 + 行数超阈值导致行高不定） */
const plainMode = computed(
  () => !useLineMode.value || (lines.value.length > VIRTUAL_THRESHOLD_LINES && settingsStore.settings.wordWrap),
)

// ---------- 虚拟滚动状态 ----------
const scrollTop = ref(0)
const viewportHeight = ref(0)
const startIndex = ref(0)
const endIndex = ref(0)
let resizeObserver: ResizeObserver | undefined

const visibleLines = computed(() => lines.value.slice(startIndex.value, endIndex.value))

function updateRange(): void {
  if (!virtualEnabled.value) return
  const total = lines.value.length
  const first = Math.max(0, Math.floor(scrollTop.value / VIRTUAL_LINE_HEIGHT) - VIRTUAL_OVERSCAN)
  const visible = Math.max(1, Math.ceil(viewportHeight.value / VIRTUAL_LINE_HEIGHT))
  const last = Math.min(total, first + visible + VIRTUAL_OVERSCAN * 2)
  startIndex.value = first
  endIndex.value = last
}

function onVirtualScroll(): void {
  const el = containerRef.value
  if (!el) return
  scrollTop.value = el.scrollTop
  updateRange()
}

watch(containerRef, (el, old) => {
  if (old) {
    old.removeEventListener('scroll', onVirtualScroll)
    resizeObserver?.disconnect()
    resizeObserver = undefined
  }
  if (el) {
    el.addEventListener('scroll', onVirtualScroll, { passive: true })
    viewportHeight.value = el.clientHeight
    updateRange()
    if (typeof ResizeObserver !== 'undefined') {
      resizeObserver = new ResizeObserver(() => {
        viewportHeight.value = el.clientHeight
        updateRange()
      })
      resizeObserver.observe(el)
    }
  }
})

// 内容变化时回到顶部并重置渲染窗口
watch(
  () => props.content,
  () => {
    scrollTop.value = 0
    startIndex.value = 0
    endIndex.value = 0
    if (containerRef.value) containerRef.value.scrollTop = 0
    void nextTick(updateRange)
  },
)

// 虚拟模式开关变化（搜索激活/关闭、换行切换）时重算渲染窗口
watch(virtualEnabled, (on) => {
  if (on) updateRange()
})

// ---------- 行数 / 字数统计（会话模式下 content 为空，由本组件上报） ----------
function emitStats(): void {
  const content = effectiveContent.value
  if (content === '') return
  emit('stats', { lines: countLines(content), words: countWords(content) })
}

watch(effectiveContent, () => emitStats(), { immediate: true })
</script>

<template>
  <div class="text-viewer">
    <div
      v-if="truncated"
      class="truncate-notice"
    >
      文件较大，当前仅预览前 {{ (MAX_PREVIEW_CHARS / 1024 / 1024).toFixed(0) }}MB 内容。
    </div>

    <div
      v-if="sessionLoading"
      class="session-notice"
    >
      正在读取大文件…
    </div>
    <div
      v-else-if="sessionError"
      class="session-error"
    >
      <p>{{ sessionError }}</p>
    </div>

    <div
      v-else
      ref="containerRef"
      class="text-content reader-scroll"
      :class="{
        'wrap-on': settingsStore.settings.wordWrap,
        'mono': settingsStore.settings.fontFamily === 'monospace',
        'virtual-scroll': virtualEnabled,
      }"
    >
      <template v-if="virtualEnabled">
        <div
          class="virtual-pad"
          :style="{ height: `${startIndex * VIRTUAL_LINE_HEIGHT}px` }"
        />
        <div
          v-for="(line, i) in visibleLines"
          :key="startIndex + i"
          class="text-line"
          :data-line="startIndex + i + 1"
        >
          <span
            v-if="showLineNumbers"
            class="line-num"
          >{{ startIndex + i + 1 }}</span>
          <span class="line-text">{{ line || ' ' }}</span>
        </div>
        <div
          class="virtual-pad"
          :style="{ height: `${(lines.length - endIndex) * VIRTUAL_LINE_HEIGHT}px` }"
        />
      </template>
      <pre
        v-else-if="plainMode"
        class="plain-pre"
      >{{ previewContent }}</pre>
      <template v-else>
        <div
          v-for="(line, index) in lines"
          :key="index"
          class="text-line"
          :data-line="index + 1"
        >
          <span
            v-if="showLineNumbers"
            class="line-num"
          >{{ index + 1 }}</span>
          <span class="line-text">{{ line || ' ' }}</span>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
/* 虚拟滚动模式：固定行高要求行内容不折行（长行横向滚动） */
.virtual-scroll {
  overflow-x: auto;
}

.virtual-scroll .line-text {
  white-space: pre;
  word-break: normal;
}

.session-notice,
.session-error {
  padding: 10px 16px;
  background: var(--bg-hover);
  border-bottom: 1px solid var(--border);
  font-size: 12.5px;
  color: var(--text-secondary);
  flex-shrink: 0;
}

.session-error {
  color: var(--danger);
}
</style>