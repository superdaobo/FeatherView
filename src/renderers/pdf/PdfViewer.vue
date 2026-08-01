<script setup lang="ts">
/**
 * PDF 渲染器（pdfjs-dist 动态导入，绝不进入首屏包）。
 *
 * Worker 方案说明：
 * 采用「主线程渲染」——不设置 GlobalWorkerOptions.workerSrc，
 * pdf.js 自动回退到 fake worker（主线程执行）。
 * 理由：Tauri 桌面端静态资源经自定义协议（tauri://localhost /
 * http://tauri.localhost）提供，WebView 跨协议加载 Worker 存在兼容性问题；
 * P0 阅读场景页数有限，主线程渲染性能足够，且彻底规避 worker URL
 * 在 dev/prod 环境不一致的问题。P0 不启用 worker，后续需要可再加。
 *
 * 数据源：props.bytesBase64（Rust read_file 返回）→ ArrayBuffer → getDocument({ data })。
 * P0 范围：翻页（上一页/下一页/页码输入）、缩放（放大/缩小/适应宽度）、
 * 加载中 / 损坏 PDF / 密码 PDF 错误提示。搜索与打印不在 P0。
 */
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { DocumentMeta } from '../../types'
import type { PDFDocumentLoadingTask, PDFDocumentProxy, RenderTask } from 'pdfjs-dist'

const props = defineProps<{
  document: DocumentMeta
  content: string
  bytesBase64?: string
}>()

type LoadStatus = 'idle' | 'loading' | 'ready' | 'error'

const status = ref<LoadStatus>('idle')
const errorMessage = ref('')
const pdfDoc = ref<PDFDocumentProxy | null>(null)
const numPages = ref(0)
const currentPage = ref(1)
const scale = ref(1)
const renderTask = ref<RenderTask | null>(null)
const stageRef = ref<HTMLElement | null>(null)
const pageCanvasRef = ref<HTMLCanvasElement | null>(null)

const MIN_SCALE = 0.5
const MAX_SCALE = 4

/** 加载竞态令牌：文档切换/卸载时使旧的异步加载失效 */
let loadToken = 0

/** base64 → ArrayBuffer（PDF 数据源来自 Rust read_file 的 bytesBase64） */
function base64ToArrayBuffer(base64: string): ArrayBuffer {
  const binary = atob(base64)
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i)
  }
  return bytes.buffer
}

/** 将 pdf.js 错误映射为面向用户的提示（按错误 name 判断，跨版本稳定） */
function describePdfError(err: unknown): string {
  let name = ''
  if (err && typeof err === 'object' && 'name' in err) {
    name = String((err as { name?: unknown }).name ?? '')
  }
  const message = err instanceof Error ? err.message : String(err)
  if (name === 'PasswordException') {
    return '该 PDF 已加密，需要密码才能打开。当前版本暂不支持输入密码，请使用其他工具打开。'
  }
  if (name === 'InvalidPDFException' || name === 'MissingPDFException') {
    return 'PDF 文件损坏或格式无效，无法解析。'
  }
  return message ? `PDF 加载失败：${message}` : 'PDF 加载失败。'
}

async function load(): Promise<void> {
  if (!props.bytesBase64) {
    status.value = 'error'
    errorMessage.value = '未提供 PDF 数据（bytesBase64 为空）。'
    return
  }
  const token = ++loadToken
  status.value = 'loading'
  errorMessage.value = ''
  pdfDoc.value = null
  numPages.value = 0
  currentPage.value = 1
  scale.value = 1
  try {
    // 动态导入：pdfjs-dist 只在 PDF 渲染器首次使用时才进入运行时（不进首屏包）
    const pdfjs = await import('pdfjs-dist')
    if (token !== loadToken) return
    const task: PDFDocumentLoadingTask = pdfjs.getDocument({
      data: base64ToArrayBuffer(props.bytesBase64),
      // WebView CSP 常禁用 eval：显式关闭 pdf.js 的 eval 依赖
      isEvalSupported: false,
    })
    const doc = await task.promise
    if (token !== loadToken) {
      void doc.destroy()
      return
    }
    pdfDoc.value = doc
    numPages.value = doc.numPages
    currentPage.value = 1
    status.value = 'ready'
    void renderPage(1)
  } catch (err) {
    if (token !== loadToken) return
    status.value = 'error'
    errorMessage.value = describePdfError(err)
  }
}

async function renderPage(pageNumber: number): Promise<void> {
  const doc = pdfDoc.value
  const canvas = pageCanvasRef.value
  if (!doc || !canvas) return
  renderTask.value?.cancel()
  renderTask.value = null
  try {
    const page = await doc.getPage(pageNumber)
    const viewport = page.getViewport({ scale: scale.value })
    canvas.width = Math.max(1, Math.floor(viewport.width))
    canvas.height = Math.max(1, Math.floor(viewport.height))
    const ctx = canvas.getContext('2d')
    if (!ctx) {
      page.cleanup()
      return
    }
    const task = page.render({ canvasContext: ctx, viewport })
    renderTask.value = task
    await task.promise
  } catch (err) {
    if (
      err && typeof err === 'object' && 'name' in err &&
      (err as { name?: unknown }).name === 'RenderingCancelledException'
    ) {
      return
    }
    console.error('[FeatherView] PDF 页面渲染失败', err)
  } finally {
    renderTask.value = null
  }
}

function setScale(next: number): void {
  scale.value = Math.min(MAX_SCALE, Math.max(MIN_SCALE, next))
  void renderPage(currentPage.value)
}

function zoomIn(): void {
  setScale(scale.value * 1.25)
}

function zoomOut(): void {
  setScale(scale.value * 0.8)
}

async function fitWidth(): Promise<void> {
  const doc = pdfDoc.value
  const stage = stageRef.value
  if (!doc || !stage) return
  const page = await doc.getPage(currentPage.value)
  const base = page.getViewport({ scale: 1 })
  const available = Math.max(stage.clientWidth - 32, 120)
  setScale(available / base.width)
}

function goToPage(raw: number): void {
  if (!pdfDoc.value || numPages.value === 0) return
  const n = Math.floor(raw)
  const clamped = Math.min(numPages.value, Math.max(1, Number.isFinite(n) ? n : 1))
  currentPage.value = clamped
  void renderPage(clamped)
}

function nextPage(): void {
  if (currentPage.value < numPages.value) goToPage(currentPage.value + 1)
}

function prevPage(): void {
  if (currentPage.value > 1) goToPage(currentPage.value - 1)
}

function onPageInput(event: Event): void {
  const el = event.target as HTMLInputElement
  goToPage(Number(el.value))
}

watch(
  () => props.document.source.id,
  () => { void load() },
)

watch(
  () => props.bytesBase64,
  () => { void load() },
)

onMounted(() => {
  void load()
})

onBeforeUnmount(() => {
  loadToken += 1
  renderTask.value?.cancel()
  renderTask.value = null
  void pdfDoc.value?.destroy()
  pdfDoc.value = null
})

defineExpose({ containerRef: stageRef })
</script>

<template>
  <div class="pdf-viewer">
    <div
      v-if="status === 'loading'"
      class="pdf-state"
    >
      <span class="pdf-state-icon">⏳</span>
      <p>正在加载 PDF…</p>
    </div>
    <div
      v-else-if="status === 'error'"
      class="pdf-state pdf-error"
    >
      <span class="pdf-state-icon">⚠️</span>
      <p class="pdf-error-title">
        无法打开 PDF
      </p>
      <p class="pdf-error-message">
        {{ errorMessage }}
      </p>
    </div>
    <template v-else-if="status === 'ready'">
      <div class="pdf-toolbar">
        <button
          class="btn btn-small"
          type="button"
          :disabled="currentPage <= 1"
          @click="prevPage"
        >
          上一页
        </button>
        <span class="pdf-page-indicator">
          <input
            class="pdf-page-input"
            type="number"
            min="1"
            :max="numPages"
            :value="currentPage"
            aria-label="页码"
            @change="onPageInput"
          >
          <span>/ {{ numPages }}</span>
        </span>
        <button
          class="btn btn-small"
          type="button"
          :disabled="currentPage >= numPages"
          @click="nextPage"
        >
          下一页
        </button>
        <span class="pdf-toolbar-sep" />
        <button
          class="btn btn-small"
          type="button"
          @click="zoomOut"
        >
          缩小
        </button>
        <span class="pdf-scale-label">{{ Math.round(scale * 100) }}%</span>
        <button
          class="btn btn-small"
          type="button"
          @click="zoomIn"
        >
          放大
        </button>
        <button
          class="btn btn-small"
          type="button"
          @click="fitWidth"
        >
          适应宽度
        </button>
        <span class="pdf-hint">P0：搜索与打印暂未支持</span>
      </div>
      <div
        ref="stageRef"
        class="pdf-stage reader-scroll"
      >
        <canvas
          ref="pageCanvasRef"
          class="pdf-canvas"
        />
      </div>
    </template>
  </div>
</template>

<style scoped>
.pdf-viewer {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.pdf-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--text-secondary);
}

.pdf-state-icon {
  font-size: 36px;
}

.pdf-error-title {
  color: var(--danger);
  font-size: 16px;
  font-weight: 600;
  margin: 0;
}

.pdf-error-message {
  margin: 0;
  max-width: 520px;
  text-align: center;
  font-size: 13px;
  color: var(--text-secondary);
}

.pdf-toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  border-bottom: 1px solid var(--border);
  background: var(--bg-toolbar);
  flex-shrink: 0;
  font-size: 12px;
}

.pdf-page-indicator {
  display: flex;
  align-items: center;
  gap: 4px;
  font-variant-numeric: tabular-nums;
  color: var(--text-secondary);
}

.pdf-page-input {
  width: 52px;
  padding: 2px 6px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg);
  color: var(--text);
  font-size: 12px;
  font-family: inherit;
  text-align: center;
}

.pdf-toolbar-sep {
  width: 1px;
  height: 18px;
  background: var(--border);
  margin: 0 4px;
}

.pdf-scale-label {
  min-width: 44px;
  text-align: center;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.pdf-hint {
  margin-left: auto;
  color: var(--text-faint);
}

.pdf-stage {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding: 16px;
  background: var(--bg);
}

.pdf-canvas {
  max-width: 100%;
  height: auto;
  box-shadow: var(--shadow);
  background: #fff;
}
</style>