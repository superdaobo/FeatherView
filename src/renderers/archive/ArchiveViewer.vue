<script setup lang="ts">
/**
 * 压缩包内容列表渲染器（P1）。
 * 依赖后端（Rust）提供 ArchiveInfo（合同 §8）；后端未实现时显示降级提示，不崩溃。
 * props 扩展约定：archive?: ArchiveInfo（主 Agent 在 types/index.ts 落地类型后可替换本地定义）。
 */
import { computed, ref } from 'vue'
import type { DocumentMeta } from '../../types'

/** 合同 §8 压缩包条目结构（与 types/index.ts 最终版保持一致） */
interface ArchiveEntry {
  path: string
  isDir: boolean
  size: number
  compressedSize?: number
  modifiedAt?: number
}

/** 合同 §8 压缩包信息结构 */
interface ArchiveInfo {
  format: 'zip' | 'tar' | 'gz' | 'tar.gz'
  entries: ArchiveEntry[]
  totalUncompressedSize: number
  truncated: boolean
}

const props = defineProps<{
  document: DocumentMeta
  content: string
  bytesBase64?: string
  archive?: ArchiveInfo
}>()

const containerRef = ref<HTMLElement | null>(null)

/** 条目渲染上限（合同上限 20000，避免 DOM 爆炸） */
const MAX_VISIBLE_ENTRIES = 500

const sortedEntries = computed<ArchiveEntry[]>(() => {
  if (!props.archive) return []
  return [...props.archive.entries].sort((a, b) => {
    if (a.isDir !== b.isDir) return a.isDir ? -1 : 1
    return a.path.localeCompare(b.path)
  })
})

const visibleEntries = computed(() => sortedEntries.value.slice(0, MAX_VISIBLE_ENTRIES))
const hiddenCount = computed(() =>
  Math.max(0, sortedEntries.value.length - MAX_VISIBLE_ENTRIES),
)

function formatSize(size: number): string {
  if (size < 1024) return `${size} B`
  const kb = size / 1024
  if (kb < 1024) return `${kb.toFixed(1)} KB`
  const mb = kb / 1024
  if (mb < 1024) return `${mb.toFixed(1)} MB`
  return `${(mb / 1024).toFixed(2)} GB`
}

function formatDate(ts?: number): string {
  if (!ts) return '—'
  const d = new Date(ts)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

function entryName(path: string): string {
  const segments = path.split('/').filter(Boolean)
  return segments[segments.length - 1] ?? path
}

defineExpose({ containerRef })
</script>

<template>
  <div class="archive-viewer">
    <template v-if="archive">
      <div class="archive-toolbar">
        <span class="archive-format">{{ archive.format.toUpperCase() }}</span>
        <span>{{ archive.entries.length }} 个条目 · 解压后共 {{ formatSize(archive.totalUncompressedSize) }}</span>
        <span
          v-if="archive.truncated"
          class="archive-truncated"
        >条目过多，列表已被后端截断</span>
      </div>
      <div
        ref="containerRef"
        class="archive-list reader-scroll"
      >
        <ul
          v-if="visibleEntries.length > 0"
          class="archive-entries"
        >
          <li
            v-for="(entry, i) in visibleEntries"
            :key="i"
            class="archive-entry"
            :class="{ dir: entry.isDir }"
          >
            <span class="archive-icon">{{ entry.isDir ? '📁' : '📄' }}</span>
            <span
              class="archive-name"
              :title="entry.path"
            >{{ entryName(entry.path) }}</span>
            <span
              v-if="!entry.isDir"
              class="archive-size"
            >{{ formatSize(entry.size) }}</span>
            <span
              v-else
              class="archive-size archive-size-dir"
            >目录</span>
            <span class="archive-date">{{ formatDate(entry.modifiedAt) }}</span>
          </li>
        </ul>
        <div
          v-else
          class="archive-empty"
        >
          压缩包内没有条目
        </div>
        <div
          v-if="hiddenCount > 0"
          class="archive-more"
        >
          仅显示前 {{ MAX_VISIBLE_ENTRIES }} 条，另有 {{ hiddenCount }} 条未显示
        </div>
      </div>
    </template>
    <div
      v-else
      class="archive-degraded"
    >
      <div class="archive-degraded-icon">
        🗜️
      </div>
      <h2>压缩包内容列表暂不可用</h2>
      <p>后端（Rust）尚未实现压缩包解析，当前版本仅能打开该文件，无法查看内部条目。</p>
      <p class="archive-degraded-note">
        该功能将在后续版本提供。
      </p>
    </div>
  </div>
</template>

<style scoped>
.archive-viewer {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.archive-toolbar {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 6px 14px;
  border-bottom: 1px solid var(--border);
  background: var(--bg-toolbar);
  font-size: 12px;
  color: var(--text-secondary);
  flex-shrink: 0;
}

.archive-format {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--accent);
  background: var(--bg-hover);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 1px 6px;
}

.archive-truncated {
  color: var(--text-faint);
}

.archive-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.archive-entries {
  list-style: none;
  margin: 0;
  padding: 6px 0;
}

.archive-entry {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 3px 14px;
  font-size: 13px;
  color: var(--text);
}

.archive-entry:hover {
  background: var(--bg-hover);
}

.archive-entry.dir .archive-name {
  font-weight: 600;
}

.archive-icon {
  flex-shrink: 0;
  font-size: 14px;
}

.archive-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.archive-size {
  flex-shrink: 0;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.archive-size-dir {
  color: var(--text-faint);
}

.archive-date {
  flex-shrink: 0;
  color: var(--text-faint);
  font-size: 12px;
}

.archive-empty {
  padding: 40px;
  text-align: center;
  color: var(--text-faint);
}

.archive-more {
  padding: 8px 14px;
  font-size: 12px;
  color: var(--text-faint);
  border-top: 1px solid var(--border);
}

.archive-degraded {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 40px;
  text-align: center;
  color: var(--text-secondary);
}

.archive-degraded-icon {
  font-size: 40px;
}

.archive-degraded h2 {
  color: var(--text);
  font-size: 18px;
  margin: 4px 0;
}

.archive-degraded-note {
  font-size: 12px;
  color: var(--text-faint);
}
</style>