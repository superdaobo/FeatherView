<script setup lang="ts">
/**
 * JSON 渲染器：格式化 / 原始切换，解析错误提示。
 */
import { computed, ref, watch } from 'vue'
import type { DocumentMeta } from '../../types'
import { hljsInstance } from '../markdown/markdown'
import { useSettingsStore } from '../../stores/settings'

const props = defineProps<{
  document: DocumentMeta
  content: string
}>()

const settingsStore = useSettingsStore()
const containerRef = ref<HTMLElement | null>(null)
const mode = ref<'formatted' | 'raw'>('formatted')

interface ParseResult {
  ok: boolean
  message?: string
  formatted?: string
}

const parseResult = computed<ParseResult>(() => {
  const raw = props.content.trim()
  if (!raw) return { ok: true, formatted: '' }
  try {
    const parsed = JSON.parse(props.content)
    return { ok: true, formatted: JSON.stringify(parsed, null, 2) }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err)
    return { ok: false, message }
  }
})

const displayHtml = computed(() => {
  if (mode.value === 'raw') {
    return hljsInstance.highlight(props.content, { language: 'json', ignoreIllegals: true }).value
  }
  if (!parseResult.value.ok || !parseResult.value.formatted) return ''
  return hljsInstance.highlight(parseResult.value.formatted, {
    language: 'json',
    ignoreIllegals: true,
  }).value
})

watch(
  () => props.content,
  () => {
    mode.value = 'formatted'
    if (containerRef.value) containerRef.value.scrollTop = 0
  },
)

defineExpose({ containerRef })
</script>

<template>
  <div class="json-viewer">
    <div class="code-toolbar">
      <span
        v-if="!parseResult.ok"
        class="json-error"
        :title="parseResult.message"
      >
        JSON 格式错误：{{ parseResult.message }}
      </span>
      <div
        class="seg"
        role="group"
        aria-label="JSON 视图模式"
      >
        <button
          type="button"
          :class="{ active: mode === 'formatted' }"
          @click="mode = 'formatted'"
        >
          格式化
        </button>
        <button
          type="button"
          :class="{ active: mode === 'raw' }"
          @click="mode = 'raw'"
        >
          原始
        </button>
      </div>
    </div>
    <div
      ref="containerRef"
      class="code-content reader-scroll"
    >
      <div
        v-if="!parseResult.ok"
        class="json-error-panel"
      >
        <p>该文件不是合法的 JSON。</p>
        <p class="json-error-detail">
          {{ parseResult.message }}
        </p>
        <button
          class="btn btn-small"
          type="button"
          @click="mode = 'raw'"
        >
          查看原始内容
        </button>
      </div>
      <pre
        v-else
        class="hljs code-pre"
        :class="{ 'wrap-on': settingsStore.settings.wordWrap }"
        v-html="displayHtml"
      />
    </div>
  </div>
</template>
