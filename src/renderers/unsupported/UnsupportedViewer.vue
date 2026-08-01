<script setup lang="ts">
/**
 * 不支持的文件类型提示。
 */
import { computed } from 'vue'
import type { DocumentMeta } from '../../types'
import { supportedExtensions } from '../registry'

const props = defineProps<{
  document: DocumentMeta
  content: string
}>()

const emit = defineEmits<{
  'reopen': []
  'home': []
}>()

const extLabel = computed(() => props.document.source.extension || '未知类型')
const supported = computed(() => supportedExtensions().join('、'))
</script>

<template>
  <div class="unsupported-viewer">
    <div class="unsupported-icon">
      📄
    </div>
    <h2>暂不支持 .{{ extLabel }} 文件</h2>
    <p>
      FeatherView 当前支持 Markdown、纯文本、代码/配置文件、JSON 与常见图片格式。
    </p>
    <p class="supported-list">
      支持：{{ supported }}
    </p>
    <div class="unsupported-actions">
      <button
        class="btn btn-primary"
        type="button"
        @click="emit('reopen')"
      >
        重新选择文件
      </button>
      <button
        class="btn"
        type="button"
        @click="emit('home')"
      >
        返回首页
      </button>
    </div>
  </div>
</template>
