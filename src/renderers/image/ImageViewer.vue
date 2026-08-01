<script setup lang="ts">
/**
 * 图片渲染器：缩放、拖动、适应窗口、原始比例、基本信息。
 * SVG 以 data URL 经 <img> 渲染，不执行其中脚本。
 */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import type { CSSProperties } from 'vue'
import type { DocumentMeta } from '../../types'
import { formatFileSize } from '../../utils'

const props = defineProps<{
  document: DocumentMeta
  content: string
  bytesBase64?: string
}>()

const containerRef = ref<HTMLElement | null>(null)
const imgRef = ref<HTMLImageElement | null>(null)
const scale = ref(1)
const fitMode = ref(true)
const offset = ref({ x: 0, y: 0 })
const naturalSize = ref<{ w: number; h: number } | null>(null)
const dragState = ref<{ startX: number; startY: number; ox: number; oy: number } | null>(null)

const ext = computed(() => props.document.source.extension.toLowerCase())

const dataUrl = computed(() => {
  if (props.bytesBase64) {
    const mimeMap: Record<string, string> = {
      png: 'image/png',
      jpg: 'image/jpeg',
      jpeg: 'image/jpeg',
      webp: 'image/webp',
      gif: 'image/gif',
      bmp: 'image/bmp',
      ico: 'image/x-icon',
      avif: 'image/avif',
    }
    return `data:${mimeMap[ext.value] ?? 'image/png'};base64,${props.bytesBase64}`
  }
  if (ext.value === 'svg' && props.content !== undefined) {
    return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(props.content)}`
  }
  return ''
})

function onImgLoad(): void {
  const img = imgRef.value
  if (img) {
    naturalSize.value = { w: img.naturalWidth, h: img.naturalHeight }
  }
}

function zoomBy(delta: number): void {
  fitMode.value = false
  scale.value = Math.min(8, Math.max(0.1, scale.value * delta))
}

function resetZoom(): void {
  scale.value = 1
  offset.value = { x: 0, y: 0 }
}

function fitWindow(): void {
  fitMode.value = true
  scale.value = 1
  offset.value = { x: 0, y: 0 }
}

function onWheel(event: WheelEvent): void {
  if (!event.ctrlKey && !event.metaKey) return
  event.preventDefault()
  zoomBy(event.deltaY > 0 ? 0.9 : 1.1)
}

function onMouseDown(event: MouseEvent): void {
  if (event.button !== 0) return
  dragState.value = {
    startX: event.clientX,
    startY: event.clientY,
    ox: offset.value.x,
    oy: offset.value.y,
  }
  window.addEventListener('mousemove', onMouseMove)
  window.addEventListener('mouseup', onMouseUp)
}

function onMouseMove(event: MouseEvent): void {
  if (!dragState.value) return
  offset.value = {
    x: dragState.value.ox + (event.clientX - dragState.value.startX),
    y: dragState.value.oy + (event.clientY - dragState.value.startY),
  }
}

function onMouseUp(): void {
  dragState.value = null
  window.removeEventListener('mousemove', onMouseMove)
  window.removeEventListener('mouseup', onMouseUp)
}

onMounted(() => {
  containerRef.value?.addEventListener('wheel', onWheel, { passive: false })
})

onBeforeUnmount(() => {
  containerRef.value?.removeEventListener('wheel', onWheel)
  window.removeEventListener('mousemove', onMouseMove)
  window.removeEventListener('mouseup', onMouseUp)
})

const imgStyle = computed<CSSProperties>(() => {
  if (fitMode.value) {
    return { maxWidth: '100%', maxHeight: '100%', objectFit: 'contain' }
  }
  return {
    transform: `translate(${offset.value.x}px, ${offset.value.y}px) scale(${scale.value})`,
    transformOrigin: 'center center',
  }
})

defineExpose({ containerRef })
</script>

<template>
  <div class="image-viewer">
    <div class="image-toolbar">
      <button
        class="btn btn-small"
        type="button"
        @click="fitWindow"
      >
        适应窗口
      </button>
      <button
        class="btn btn-small"
        type="button"
        @click="zoomBy(1.25)"
      >
        放大
      </button>
      <button
        class="btn btn-small"
        type="button"
        @click="zoomBy(0.8)"
      >
        缩小
      </button>
      <button
        class="btn btn-small"
        type="button"
        @click="resetZoom"
      >
        原始比例
      </button>
      <span class="image-hint">Ctrl + 滚轮缩放 · 拖动查看</span>
    </div>
    <div
      ref="containerRef"
      class="image-stage reader-scroll"
    >
      <img
        v-if="dataUrl"
        ref="imgRef"
        class="image-canvas"
        :style="imgStyle"
        :src="dataUrl"
        :draggable="false"
        alt=""
        @load="onImgLoad"
        @mousedown="onMouseDown"
      >
      <div
        v-else
        class="image-error"
      >
        图片数据加载失败
      </div>
    </div>
    <div
      v-if="naturalSize"
      class="image-info"
    >
      {{ naturalSize.w }} × {{ naturalSize.h }} px ·
      {{ ext.toUpperCase() }} ·
      {{ formatFileSize(document.size) }} ·
      缩放 {{ Math.round((fitMode ? 1 : scale) * 100) }}%
    </div>
  </div>
</template>
