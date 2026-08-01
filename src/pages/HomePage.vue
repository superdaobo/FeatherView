<script setup lang="ts">
/**
 * 首页：产品介绍、打开文件、最近文件列表、支持格式说明。
 */
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { FolderOpen, History, Trash2, X } from 'lucide-vue-next'
import { useDocumentStore } from '../stores/document'
import { useRecentFilesStore } from '../stores/recentFiles'
import type { DocumentSource, RecentFile } from '../types'
import { pickFile } from '../services/platformService'
import { checkFileExists } from '../services/documentService'
import { useToast } from '../composables/useToast'
import { useShortcuts } from '../composables/useShortcuts'
import { formatDate, formatFileSize } from '../utils'

const router = useRouter()
const documentStore = useDocumentStore()
const recentStore = useRecentFilesStore()
const { show } = useToast()

const opening = ref(false)
const invalidPaths = ref(new Set<string>())

const formatGroups = [
  { title: 'Markdown', items: '.md · .markdown · .mdown' },
  { title: '纯文本 / 日志', items: '.txt · .log' },
  { title: '代码与配置', items: '.js .ts .vue .css .html .py .rs .java .c .cpp .go .sh .ps1 等' },
  { title: '结构化数据', items: '.json · .yaml · .toml · .xml · .ini · .env' },
  { title: '图片', items: '.png .jpg .jpeg .webp .gif .svg 等' },
]

async function openSource(source: DocumentSource): Promise<void> {
  opening.value = true
  try {
    const ok = await documentStore.open(source)
    if (ok) {
      void router.push('/reader')
    } else {
      show(documentStore.error?.title ?? '打开失败', {
        message: documentStore.error?.message,
        kind: 'error',
        duration: 6000,
      })
    }
  } finally {
    opening.value = false
  }
}

async function openFromPicker(): Promise<void> {
  const source = await pickFile()
  if (source) await openSource(source)
}

async function openRecent(file: RecentFile): Promise<void> {
  const exists = await checkFileExists(file.path)
  if (!exists) {
    invalidPaths.value = new Set(invalidPaths.value).add(file.path)
    show(`文件已失效：${file.name}`, {
      message: '该文件可能已被移动、重命名或删除。你可以删除这条记录。',
      kind: 'error',
      duration: 5000,
    })
    return
  }
  await openSource(recentStore.toSource(file))
}

function removeRecent(file: RecentFile): void {
  recentStore.remove(file.path)
  const next = new Set(invalidPaths.value)
  next.delete(file.path)
  invalidPaths.value = next
}

function clearAll(): void {
  if (recentStore.files.length === 0) return
  if (window.confirm('确定清空所有最近打开记录吗？')) {
    recentStore.clearAll()
    invalidPaths.value = new Set()
  }
}

useShortcuts({
  onOpenFile: () => {
    void openFromPicker()
  },
})
</script>

<template>
  <div class="home-page">
    <header class="home-header">
      <div class="brand">
        <div class="brand-logo">
          🪶
        </div>
        <div>
          <h1>FeatherView <span class="brand-cn">轻阅</span></h1>
          <p class="brand-desc">
            轻量、本地优先的 Markdown 与文件阅读器
          </p>
        </div>
      </div>
      <nav class="home-nav">
        <router-link
          to="/settings"
          class="nav-link"
        >
          设置
        </router-link>
      </nav>
    </header>

    <main class="home-main">
      <section class="hero">
        <button
          class="btn btn-primary btn-open"
          type="button"
          :disabled="opening"
          @click="openFromPicker"
        >
          <FolderOpen :size="18" />
          打开文件
          <kbd class="kbd">Ctrl+O</kbd>
        </button>
        <p class="hero-hint">
          或将文件拖入窗口 · 本地处理，不会上传任何内容
        </p>
      </section>

      <section class="recent-section">
        <div class="section-head">
          <h2 class="section-title">
            <History :size="16" /> 最近打开
          </h2>
          <button
            v-if="recentStore.files.length > 0"
            class="btn-icon"
            type="button"
            title="清空最近记录"
            @click="clearAll"
          >
            <Trash2 :size="16" />
          </button>
        </div>

        <div
          v-if="recentStore.files.length === 0"
          class="empty-state"
        >
          <div class="empty-icon">
            📂
          </div>
          <p>还没有打开过文件</p>
          <p class="empty-hint">
            点击上方按钮选择文件，或直接把文件拖进窗口
          </p>
        </div>

        <ul
          v-else
          class="recent-list"
        >
          <li
            v-for="file in recentStore.files"
            :key="file.path"
            class="recent-item"
            :class="{ invalid: invalidPaths.has(file.path) }"
          >
            <button
              class="recent-main"
              type="button"
              @click="openRecent(file)"
            >
              <span class="recent-ext">.{{ file.extension || '?' }}</span>
              <span class="recent-info">
                <span class="recent-name">{{ file.name }}</span>
                <span class="recent-meta">
                  {{ formatDate(file.lastOpenedAt) }}
                  <template v-if="file.size !== undefined"> · {{ formatFileSize(file.size) }}</template>
                  <template v-if="invalidPaths.has(file.path)"> · <span class="invalid-label">文件不存在</span></template>
                </span>
              </span>
            </button>
            <button
              class="btn-icon recent-remove"
              type="button"
              :title="invalidPaths.has(file.path) ? '删除记录' : '从最近列表移除'"
              @click="removeRecent(file)"
            >
              <X :size="15" />
            </button>
          </li>
        </ul>
      </section>

      <section class="formats-section">
        <h2 class="section-title">
          支持的文件类型
        </h2>
        <div class="formats-grid">
          <div
            v-for="group in formatGroups"
            :key="group.title"
            class="format-card"
          >
            <h3>{{ group.title }}</h3>
            <p>{{ group.items }}</p>
          </div>
        </div>
      </section>
    </main>

    <footer class="home-footer">
      FeatherView v0.1.0 · 本地离线阅读 · 无账号无广告
    </footer>
  </div>
</template>

<style scoped>
.home-page {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.home-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 18px 28px 8px;
}

.brand {
  display: flex;
  align-items: center;
  gap: 12px;
}

.brand-logo {
  font-size: 30px;
}

.brand h1 {
  margin: 0;
  font-size: 20px;
  font-weight: 700;
}

.brand-cn {
  font-weight: 400;
  color: var(--text-secondary);
  font-size: 15px;
  margin-left: 4px;
}

.brand-desc {
  margin: 2px 0 0;
  color: var(--text-secondary);
  font-size: 12px;
}

.nav-link {
  color: var(--text-secondary);
  text-decoration: none;
  font-size: 13px;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
}

.nav-link:hover {
  color: var(--text);
  background: var(--bg-hover);
}

.home-main {
  flex: 1;
  overflow-y: auto;
  padding: 12px 28px 24px;
  max-width: 860px;
  width: 100%;
  margin: 0 auto;
}

.hero {
  padding: 28px 0 16px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}

.btn-open {
  padding: 12px 26px;
  font-size: 15px;
  border-radius: var(--radius);
}

.kbd {
  margin-left: 8px;
  font-family: var(--font-mono);
  font-size: 11px;
  padding: 2px 6px;
  border: 1px solid rgba(255, 255, 255, 0.4);
  border-radius: 4px;
  opacity: 0.85;
}

.hero-hint {
  color: var(--text-faint);
  font-size: 12px;
  margin: 0;
}

.recent-section {
  margin-top: 18px;
}

.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.section-title {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 14px;
  font-weight: 600;
  margin: 0;
  color: var(--text-secondary);
}

.empty-hint {
  color: var(--text-faint);
  font-size: 12px;
  margin: 0;
}

.recent-list {
  list-style: none;
  margin: 0;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  overflow: hidden;
  background: var(--bg-panel);
}

.recent-item {
  display: flex;
  align-items: center;
  border-bottom: 1px solid var(--border);
}

.recent-item:last-child {
  border-bottom: none;
}

.recent-item:hover {
  background: var(--bg-hover);
}

.recent-item.invalid .recent-name {
  color: var(--danger);
}

.recent-main {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  border: none;
  background: none;
  cursor: pointer;
  text-align: left;
  min-width: 0;
}

.recent-ext {
  flex-shrink: 0;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--accent);
  background: var(--bg-hover);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 2px 6px;
  max-width: 90px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.recent-info {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.recent-name {
  font-size: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.recent-meta {
  font-size: 11.5px;
  color: var(--text-faint);
  margin-top: 2px;
}

.invalid-label {
  color: var(--danger);
}

.recent-remove {
  margin-right: 8px;
  flex-shrink: 0;
}

.formats-section {
  margin-top: 26px;
}

.formats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
  gap: 10px;
  margin-top: 10px;
}

.format-card {
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 10px 14px;
  background: var(--bg-panel);
}

.format-card h3 {
  margin: 0 0 4px;
  font-size: 13px;
  font-weight: 600;
}

.format-card p {
  margin: 0;
  font-size: 12px;
  color: var(--text-secondary);
  font-family: var(--font-mono);
}

.home-footer {
  text-align: center;
  padding: 8px;
  color: var(--text-faint);
  font-size: 11.5px;
  border-top: 1px solid var(--border);
  flex-shrink: 0;
}
</style>
