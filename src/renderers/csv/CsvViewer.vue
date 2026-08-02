<script setup lang="ts">
/**
 * CSV 渲染器：轻量解析（无第三方库），表格视图。
 * 支持：分隔符自动检测（逗号/分号/制表符）、引号字段、引号内换行、
 * 表头加粗、行列计数、超 5000 行截断提示。
 */
import { computed, ref, watch } from 'vue'
import type { DocumentMeta } from '../../types'
import { MAX_CSV_ROWS, parseCsv } from './csvParser'

const props = defineProps<{
  document: DocumentMeta
  content: string
}>()

const containerRef = ref<HTMLElement | null>(null)

/** 单格渲染长度上限，防止超长字段撑爆 DOM */
const MAX_CELL_CHARS = 500

const parse = computed(() => parseCsv(props.content))

const rowCount = computed(() => parse.value.rows.length)
const colCount = computed(() =>
  Math.max(parse.value.header.length, parse.value.rows[0]?.length ?? 0),
)
const delimiterLabel = computed(() =>
  parse.value.delimiter === '\t' ? 'Tab' : parse.value.delimiter,
)

function cellText(value: string): string {
  if (value.length <= MAX_CELL_CHARS) return value
  return `${value.slice(0, MAX_CELL_CHARS)}…（字段过长已截断）`
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
  <div class="csv-viewer">
    <div class="csv-toolbar">
      <span class="csv-stats">{{ rowCount }} 行 × {{ colCount }} 列 · 分隔符 {{ delimiterLabel }}</span>
      <span
        v-if="parse.truncated"
        class="csv-truncate"
        title="为避免大文件卡顿，仅解析并显示前 5000 行数据"
      >
        文件超过 {{ MAX_CSV_ROWS }} 行，仅显示前 {{ MAX_CSV_ROWS }} 行（共约 {{ parse.estimatedTotalRows }} 行）
      </span>
      <span
        v-for="(issue, i) in parse.issues"
        :key="i"
        class="csv-issue"
      >{{ issue }}</span>
    </div>
    <div
      ref="containerRef"
      class="csv-scroll reader-scroll"
    >
      <div
        v-if="rowCount === 0 && parse.header.length === 0"
        class="csv-empty"
      >
        空文件
      </div>
      <table
        v-else
        class="csv-table"
      >
        <thead v-if="parse.header.length > 0">
          <tr>
            <th
              v-for="(cell, i) in parse.header"
              :key="i"
            >
              {{ cellText(cell) }}
            </th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="(row, ri) in parse.rows"
            :key="ri"
          >
            <td
              v-for="(cell, ci) in row"
              :key="ci"
            >
              {{ cellText(cell) }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.csv-viewer {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.csv-toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 4px 14px;
  padding: 6px 14px;
  border-bottom: 1px solid var(--border);
  background: var(--bg-toolbar);
  font-size: 12px;
  color: var(--text-secondary);
  flex-shrink: 0;
}

.csv-stats {
  font-variant-numeric: tabular-nums;
}

.csv-truncate {
  font-weight: 600;
  color: var(--text);
}

.csv-issue {
  color: var(--text-faint);
}

.csv-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.csv-table {
  border-collapse: collapse;
  width: 100%;
  font-size: 13px;
}

.csv-table th,
.csv-table td {
  border: 1px solid var(--border);
  padding: 4px 10px;
  text-align: left;
  vertical-align: top;
  max-width: 480px;
  white-space: pre-wrap;
  word-break: break-all;
}

.csv-table th {
  font-weight: 600;
  background: var(--bg-hover);
  position: sticky;
  top: 0;
}

.csv-empty {
  padding: 40px;
  text-align: center;
  color: var(--text-faint);
}
</style>