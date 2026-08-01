<script setup lang="ts">
/**
 * 纯文本渲染器（txt / log）。
 * 支持：行号、自动换行、等宽字体、大文本截断保护。
 */
import { computed, ref, watch } from 'vue'
import type { DocumentMeta } from '../../types'
import { useSettingsStore } from '../../stores/settings'

const props = defineProps<{
  document: DocumentMeta
  content: string
}>()

const settingsStore = useSettingsStore()
const containerRef = ref<HTMLElement | null>(null)

/** 大文本截断保护阈值（字符数） */
const MAX_PREVIEW_CHARS = 2_000_000
/** 行渲染模式阈值：超过后不逐行渲染（避免 DOM 节点爆炸） */
const MAX_LINE_MODE_CHARS = 300_000

const truncated = computed(() => props.content.length > MAX_PREVIEW_CHARS)
const previewContent = computed(() =>
  truncated.value ? props.content.slice(0, MAX_PREVIEW_CHARS) : props.content,
)

const useLineMode = computed(
  () => previewContent.value.length <= MAX_LINE_MODE_CHARS,
)

const lines = computed(() => {
  if (!useLineMode.value) return []
  return previewContent.value.split(/\r\n|\r|\n/)
})

const showLineNumbers = computed(() => {
  return settingsStore.settings.showLineNumbers && useLineMode.value
})

watch(
  () => props.content,
  () => {
    // 内容变化时回到顶部
    if (containerRef.value) containerRef.value.scrollTop = 0
  },
)

defineExpose({ containerRef })
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
      ref="containerRef"
      class="text-content reader-scroll"
      :class="{
        'wrap-on': settingsStore.settings.wordWrap,
        'mono': settingsStore.settings.fontFamily === 'monospace',
      }"
    >
      <template v-if="useLineMode">
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
      <pre
        v-else
        class="plain-pre"
      >{{ previewContent }}</pre>
    </div>
  </div>
</template>
