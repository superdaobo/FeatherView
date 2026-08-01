<script setup lang="ts">
/**
 * 阅读页面：三栏自适应布局。
 * 左栏：文件信息与最近文件（可收起）
 * 中栏：阅读区（渲染器分发）
 * 右栏：Markdown 目录（可收起）
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import {
  ArrowLeft,
  FileText,
  Search,
  ListTree,
  Sun,
  Moon,
  Monitor,
  Plus,
  Minus,
  Settings,
  FolderOpen,
  ChevronUp,
  ChevronDown,
  X,
  Copy,
  PanelLeft,
} from 'lucide-vue-next'
import { useDocumentStore } from '../stores/document'
import { useSettingsStore } from '../stores/settings'
import { useRecentFilesStore } from '../stores/recentFiles'
import { matchRenderer } from '../renderers/registry'
import type { RendererDefinition } from '../types'
import { useShortcuts } from '../composables/useShortcuts'
import { useDocumentSearch } from '../composables/useDocumentSearch'
import { useToast } from '../composables/useToast'
import { useReadingPosition } from '../composables/useReadingPosition'
import { pickFile } from '../services/platformService'
import { checkFileExists } from '../services/documentService'
import { DEFAULT_WINDOW_TITLE, setWindowTitle } from '../services/windowService'
import { countLines, countWords, formatDate, formatFileSize } from '../utils'
import type { RecentFile } from '../types'

const router = useRouter()
const documentStore = useDocumentStore()
const settingsStore = useSettingsStore()
const recentStore = useRecentFilesStore()
const { show } = useToast()

// ---------- 渲染器 ----------
const renderer = computed<RendererDefinition | undefined>(() => {
  if (!documentStore.meta) return undefined
  return matchRenderer(documentStore.meta)
})

const rendererRef = ref<{ containerRef?: HTMLElement | null } | null>(null)

/** 渲染器内容容器（搜索/进度基于它） */
const contentContainer = computed(() => rendererRef.value?.containerRef ?? null)

const toc = ref<Array<{ level: number; text: string; id: string }>>([])

function onToc(newToc: Array<{ level: number; text: string; id: string }>): void {
  toc.value = newToc
}

// ---------- 搜索 ----------
const search = useDocumentSearch(contentContainer)

function runSearchAfterRender(): void {
  // 等 DOM 更新后执行搜索（内容变化或渲染器挂载时）
  requestAnimationFrame(() => {
    if (search.active.value) search.runSearch()
  })
}

watch(
  () => documentStore.result?.content,
  () => runSearchAfterRender(),
)

// ---------- 阅读进度 ----------
const progress = ref(0)

function onScroll(): void {
  const el = contentContainer.value
  if (!el) return
  const max = el.scrollHeight - el.clientHeight
  progress.value = max > 0 ? Math.min(100, Math.round((el.scrollTop / max) * 100)) : 0
}

watch(contentContainer, (el, old) => {
  old?.removeEventListener('scroll', onScroll)
  el?.addEventListener('scroll', onScroll, { passive: true })
  onScroll()
})

// ---------- 侧栏 ----------
const leftOpen = ref(true)
const rightOpen = ref(true)
const narrowMode = ref(false)
const drawerSide = ref<'left' | 'right' | null>(null)

function checkNarrow(): void {
  narrowMode.value = window.innerWidth < 1080
}

onMounted(() => {
  checkNarrow()
  window.addEventListener('resize', checkNarrow)
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', checkNarrow)
})

const showLeft = computed(() => (narrowMode.value ? drawerSide.value === 'left' : leftOpen.value))
const showRight = computed(() => (narrowMode.value ? drawerSide.value === 'right' : rightOpen.value))

function toggleLeft(): void {
  if (narrowMode.value) {
    drawerSide.value = drawerSide.value === 'left' ? null : 'left'
  } else {
    leftOpen.value = !leftOpen.value
  }
}

function toggleRight(): void {
  if (narrowMode.value) {
    drawerSide.value = drawerSide.value === 'right' ? null : 'right'
  } else {
    rightOpen.value = !rightOpen.value
  }
}

// ---------- 阅读位置 ----------
const position = useReadingPosition(
  contentContainer,
  computed(() => documentStore.source),
  computed(() => documentStore.meta),
  computed(() => renderer.value?.id),
  computed(() => settingsStore.settings.rememberReadingPosition),
  () => {
    show('已恢复上次阅读位置', { kind: 'success', duration: 2000 })
  },
)

// ---------- 窗口标题 ----------
watch(
  () => documentStore.source?.name,
  (name) => {
    setWindowTitle(name ? `${name} — FeatherView` : DEFAULT_WINDOW_TITLE)
  },
  { immediate: true },
)

// ---------- 目录跳转 ----------
function jumpToHeading(id: string): void {
  const el = contentContainer.value
  if (!el) return
  const target = el.querySelector(`#${CSS.escape(id)}`)
  if (target) {
    target.scrollIntoView({ behavior: 'smooth', block: 'start' })
    if (narrowMode.value) drawerSide.value = null
  }
}

// ---------- 文件操作 ----------
async function openFromPicker(): Promise<void> {
  position.saveNow()
  const source = await pickFile()
  if (source) {
    const ok = await documentStore.open(source)
    if (ok) {
      toc.value = []
      runSearchAfterRender()
    }
  }
}

async function openRecent(file: RecentFile): Promise<void> {
  const exists = await checkFileExists(file.path)
  if (!exists) {
    show(`文件已失效：${file.name}`, {
      message: '该文件可能已被移动或删除。',
      kind: 'error',
    })
    return
  }
  position.saveNow()
  await documentStore.open(recentStore.toSource(file))
  toc.value = []
  runSearchAfterRender()
}

function copyErrorDetail(): void {
  if (!documentStore.error) return
  void navigator.clipboard
    .writeText(`[${documentStore.error.code}] ${documentStore.error.title}\n${documentStore.error.message}`)
    .then(() => show('错误详情已复制', { kind: 'success' }))
    .catch(() => show('复制失败', { kind: 'error' }))
}

function goHome(): void {
  void router.push('/')
}

function goSettings(): void {
  void router.push('/settings')
}

// ---------- 状态栏数据 ----------
const lineCount = computed(() => {
  const content = documentStore.result?.content
  return content !== undefined ? countLines(content) : 0
})

const wordCount = computed(() => {
  const content = documentStore.result?.content
  return content !== undefined ? countWords(content) : 0
})

// ---------- 快捷键 ----------
useShortcuts({
  onOpenFile: () => {
    void openFromPicker()
  },
  onSearch: () => {
    if (documentStore.isReady) search.open()
  },
  onFontIncrease: () => settingsStore.adjustFontSize(1),
  onFontDecrease: () => settingsStore.adjustFontSize(-1),
  onFontReset: () => settingsStore.resetFontSize(),
  onToggleTheme: () => settingsStore.toggleTheme(),
  onEscape: () => {
    if (search.active.value) {
      search.close()
    } else if (drawerSide.value) {
      drawerSide.value = null
    }
  },
})

// 渲染器组件实例类型（动态组件，事件透传）
type RendererInstance = { containerRef?: HTMLElement | null }

function onRendererMounted(instance: RendererInstance | null): void {
  rendererRef.value = instance
}
</script>

<template>
  <div class="reader-page">
    <!-- 顶部工具栏 -->
    <header class="toolbar reader-toolbar">
      <button
        class="btn-icon"
        type="button"
        title="返回首页"
        @click="goHome"
      >
        <ArrowLeft :size="18" />
      </button>
      <span
        class="toolbar-title"
        :title="documentStore.source?.path ?? ''"
      >
        {{ documentStore.source?.name ?? '阅读器' }}
      </span>
      <span
        v-if="documentStore.source?.extension"
        class="ext-badge"
      >
        .{{ documentStore.source.extension }}
      </span>

      <div class="toolbar-spacer" />

      <button
        class="btn-icon"
        type="button"
        title="文件信息 / 最近文件"
        :class="{ active: showLeft }"
        @click="toggleLeft"
      >
        <PanelLeft :size="17" />
      </button>
      <button
        class="btn-icon"
        type="button"
        title="搜索 (Ctrl+F)"
        :class="{ active: search.active.value }"
        @click="search.open()"
      >
        <Search :size="17" />
      </button>
      <button
        v-if="renderer?.id === 'markdown' && toc.length > 0"
        class="btn-icon"
        type="button"
        title="目录"
        :class="{ active: showRight }"
        @click="toggleRight"
      >
        <ListTree :size="17" />
      </button>
      <button
        class="btn-icon"
        type="button"
        title="主题切换 (Ctrl+Shift+T)"
        @click="settingsStore.toggleTheme()"
      >
        <Sun
          v-if="settingsStore.settings.theme === 'light'"
          :size="17"
        />
        <Moon
          v-else-if="settingsStore.settings.theme === 'dark'"
          :size="17"
        />
        <Monitor
          v-else
          :size="17"
        />
      </button>
      <div class="font-controls">
        <button
          class="btn-icon"
          type="button"
          title="减小字号 (Ctrl+-)"
          @click="settingsStore.adjustFontSize(-1)"
        >
          <Minus :size="16" />
        </button>
        <span class="font-size-label">{{ settingsStore.settings.fontSize }}</span>
        <button
          class="btn-icon"
          type="button"
          title="增大字号 (Ctrl++)"
          @click="settingsStore.adjustFontSize(1)"
        >
          <Plus :size="16" />
        </button>
      </div>
      <button
        class="btn-icon"
        type="button"
        title="打开文件 (Ctrl+O)"
        :disabled="documentStore.isLoading"
        @click="openFromPicker"
      >
        <FolderOpen :size="17" />
      </button>
      <button
        class="btn-icon"
        type="button"
        title="设置"
        @click="goSettings"
      >
        <Settings :size="17" />
      </button>
    </header>

    <div class="reader-body">
      <!-- 左栏：文件信息 + 最近文件 -->
      <aside
        v-if="showLeft"
        class="sidebar sidebar-left"
        :class="{ drawer: narrowMode }"
      >
        <div
          v-if="narrowMode"
          class="drawer-head"
        >
          <span>文件信息</span>
          <button
            class="btn-icon"
            type="button"
            @click="drawerSide = null"
          >
            <X :size="15" />
          </button>
        </div>
        <div
          v-if="documentStore.meta"
          class="file-info"
        >
          <h3 class="sidebar-title">
            <FileText :size="14" /> 文件信息
          </h3>
          <dl class="info-list">
            <div class="info-row">
              <dt>名称</dt>
              <dd :title="documentStore.source?.name">
                {{ documentStore.source?.name }}
              </dd>
            </div>
            <div
              v-if="documentStore.source?.path"
              class="info-row"
            >
              <dt>路径</dt>
              <dd :title="documentStore.source.path">
                {{ documentStore.source.path }}
              </dd>
            </div>
            <div class="info-row">
              <dt>大小</dt>
              <dd>{{ formatFileSize(documentStore.meta.size) }}</dd>
            </div>
            <div
              v-if="documentStore.meta.encoding"
              class="info-row"
            >
              <dt>编码</dt>
              <dd>{{ documentStore.meta.encoding }}</dd>
            </div>
            <div
              v-if="documentStore.meta.modifiedAt"
              class="info-row"
            >
              <dt>修改时间</dt>
              <dd>{{ formatDate(documentStore.meta.modifiedAt) }}</dd>
            </div>
          </dl>
        </div>
        <div class="recent-side">
          <h3 class="sidebar-title">
            最近文件
          </h3>
          <ul class="side-list">
            <li
              v-for="file in recentStore.files.slice(0, 10)"
              :key="file.path"
            >
              <button
                class="side-item"
                type="button"
                :title="file.path"
                @click="openRecent(file)"
              >
                <span class="side-ext">.{{ file.extension || '?' }}</span>
                <span class="side-name">{{ file.name }}</span>
              </button>
            </li>
            <li
              v-if="recentStore.files.length === 0"
              class="side-empty"
            >
              暂无最近文件
            </li>
          </ul>
        </div>
      </aside>
      <div
        v-if="narrowMode && showLeft"
        class="drawer-mask"
        @click="drawerSide = null"
      />

      <!-- 中栏：阅读区 -->
      <main class="reader-main">
        <!-- 加载中 -->
        <div
          v-if="documentStore.isLoading"
          class="empty-state"
        >
          <div class="empty-icon">
            ⏳
          </div>
          <p>正在读取文件…</p>
        </div>

        <!-- 错误面板 -->
        <div
          v-else-if="documentStore.status === 'error' && documentStore.error"
          class="error-panel"
        >
          <h2 class="error-title">
            {{ documentStore.error.title }}
          </h2>
          <p class="error-message">
            {{ documentStore.error.message }}
          </p>
          <div class="error-actions">
            <button
              class="btn btn-primary"
              type="button"
              @click="openFromPicker"
            >
              <FolderOpen :size="15" /> 重新选择文件
            </button>
            <button
              class="btn"
              type="button"
              @click="goHome"
            >
              返回首页
            </button>
            <button
              class="btn"
              type="button"
              @click="copyErrorDetail"
            >
              <Copy :size="14" /> 复制错误详情
            </button>
          </div>
        </div>

        <!-- 渲染器 -->
        <template v-else-if="documentStore.isReady && documentStore.meta && documentStore.result">
          <component
            :is="renderer?.component"
            :ref="onRendererMounted"
            :document="documentStore.meta"
            :content="documentStore.result.content ?? ''"
            :bytes-base64="documentStore.result.bytesBase64"
            @toc="onToc"
            @reopen="openFromPicker"
            @home="goHome"
          />
        </template>

        <div
          v-else
          class="empty-state"
        >
          <div class="empty-icon">
            📄
          </div>
          <p>没有打开的文件</p>
          <button
            class="btn btn-primary"
            type="button"
            @click="openFromPicker"
          >
            打开文件
          </button>
        </div>

        <!-- 搜索栏 -->
        <div
          v-if="search.active.value"
          class="search-bar"
        >
          <input
            v-model="search.query.value"
            class="search-input"
            type="text"
            placeholder="在文档中搜索…"
            autofocus
            @input="search.runSearch()"
            @keydown.enter="search.next()"
          >
          <span class="search-count">
            <template v-if="search.count.value > 0">
              {{ search.currentIndex.value + 1 }} / {{ search.count.value }}
            </template>
            <template v-else>无结果</template>
          </span>
          <button
            class="btn-icon"
            type="button"
            title="上一个"
            @click="search.prev()"
          >
            <ChevronUp :size="15" />
          </button>
          <button
            class="btn-icon"
            type="button"
            title="下一个"
            @click="search.next()"
          >
            <ChevronDown :size="15" />
          </button>
          <button
            class="btn-icon"
            type="button"
            title="关闭 (Esc)"
            @click="search.close()"
          >
            <X :size="15" />
          </button>
        </div>
      </main>

      <!-- 右栏：目录 -->
      <aside
        v-if="showRight"
        class="sidebar sidebar-right"
        :class="{ drawer: narrowMode }"
      >
        <div
          v-if="narrowMode"
          class="drawer-head"
        >
          <span>目录</span>
          <button
            class="btn-icon"
            type="button"
            @click="drawerSide = null"
          >
            <X :size="15" />
          </button>
        </div>
        <h3 class="sidebar-title">
          目录
        </h3>
        <ul class="toc-list">
          <li
            v-for="item in toc"
            :key="item.id"
          >
            <button
              class="toc-item"
              :class="`toc-level-${item.level}`"
              type="button"
              :title="item.text"
              @click="jumpToHeading(item.id)"
            >
              {{ item.text }}
            </button>
          </li>
        </ul>
      </aside>
      <div
        v-if="narrowMode && showRight"
        class="drawer-mask"
        @click="drawerSide = null"
      />
    </div>

    <!-- 底部状态栏 -->
    <footer class="statusbar">
      <span v-if="documentStore.meta">{{ formatFileSize(documentStore.meta.size) }}</span>
      <span v-if="documentStore.meta?.encoding">· {{ documentStore.meta.encoding }}</span>
      <span v-if="lineCount > 0">· {{ lineCount }} 行 / {{ wordCount }} 字</span>
      <span class="statusbar-spacer" />
      <span v-if="search.active.value && search.count.value > 0">
        搜索 {{ search.currentIndex.value + 1 }}/{{ search.count.value }}
      </span>
      <span v-if="documentStore.isReady">阅读进度 {{ progress }}%</span>
      <span v-else-if="documentStore.status === 'error'">打开失败</span>
    </footer>
  </div>
</template>

<style scoped>
.reader-page {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.reader-toolbar {
  gap: 4px;
}

.ext-badge {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--accent);
  background: var(--bg-hover);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 2px 7px;
  flex-shrink: 0;
}

.font-controls {
  display: flex;
  align-items: center;
  gap: 2px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 1px 4px;
}

.font-size-label {
  font-size: 11.5px;
  color: var(--text-secondary);
  min-width: 22px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}

.reader-body {
  flex: 1;
  display: flex;
  min-height: 0;
  position: relative;
}

.sidebar {
  width: var(--sidebar-width);
  flex-shrink: 0;
  background: var(--bg-panel);
  border-right: 1px solid var(--border);
  overflow-y: auto;
  padding: 12px;
}

.sidebar-right {
  border-right: none;
  border-left: 1px solid var(--border);
}

.sidebar.drawer {
  position: absolute;
  top: 0;
  bottom: 0;
  z-index: 100;
  box-shadow: var(--shadow);
}

.sidebar-left.drawer {
  left: 0;
}

.sidebar-right.drawer {
  right: 0;
}

.drawer-mask {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.3);
  z-index: 90;
}

.drawer-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
  font-weight: 600;
  font-size: 13px;
}

.sidebar-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.4px;
  margin: 4px 0 8px;
}

.file-info {
  padding-bottom: 12px;
  border-bottom: 1px solid var(--border);
}

.info-list {
  margin: 0;
}

.info-row {
  display: flex;
  gap: 8px;
  font-size: 12px;
  padding: 3px 0;
}

.info-row dt {
  color: var(--text-faint);
  flex-shrink: 0;
  width: 48px;
}

.info-row dd {
  margin: 0;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-secondary);
}

.recent-side {
  margin-top: 12px;
}

.side-list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.side-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 6px 8px;
  border: none;
  background: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: 12.5px;
  color: var(--text);
  text-align: left;
}

.side-item:hover {
  background: var(--bg-hover);
}

.side-ext {
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--text-faint);
  flex-shrink: 0;
}

.side-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.side-empty {
  color: var(--text-faint);
  font-size: 12px;
  padding: 6px 8px;
}

.reader-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  position: relative;
  background: var(--bg);
}

/* 错误面板 */
.error-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 40px;
  text-align: center;
}

.error-title {
  color: var(--danger);
  font-size: 18px;
  margin: 0;
}

.error-message {
  color: var(--text-secondary);
  max-width: 520px;
  white-space: pre-wrap;
  word-break: break-word;
  margin: 0;
}

.error-actions {
  display: flex;
  gap: 10px;
  margin-top: 8px;
  flex-wrap: wrap;
  justify-content: center;
}

/* 搜索栏 */
.search-bar {
  position: absolute;
  top: 10px;
  right: 14px;
  z-index: 50;
  display: flex;
  align-items: center;
  gap: 2px;
  background: var(--bg-panel);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius);
  box-shadow: var(--shadow);
  padding: 4px 6px;
}

.search-input {
  border: none;
  outline: none;
  background: transparent;
  color: var(--text);
  font-size: 13px;
  width: 200px;
  padding: 4px 6px;
  font-family: inherit;
}

.search-count {
  font-size: 11.5px;
  color: var(--text-secondary);
  min-width: 48px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}

/* 目录 */
.toc-list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.toc-item {
  display: block;
  width: 100%;
  border: none;
  background: none;
  cursor: pointer;
  color: var(--text-secondary);
  font-size: 12.5px;
  padding: 4px 8px;
  border-radius: var(--radius-sm);
  text-align: left;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.toc-item:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.toc-level-1 {
  font-weight: 600;
  color: var(--text);
  padding-left: 8px;
}

.toc-level-2 {
  padding-left: 20px;
}

.toc-level-3 {
  padding-left: 32px;
  font-size: 12px;
}

/* 状态栏 */
.statusbar {
  height: var(--statusbar-height);
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 14px;
  font-size: 11.5px;
  color: var(--text-secondary);
  background: var(--bg-toolbar);
  border-top: 1px solid var(--border);
  white-space: nowrap;
  overflow: hidden;
}

.statusbar-spacer {
  flex: 1;
}
</style>
