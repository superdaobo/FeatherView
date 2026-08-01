<script setup lang="ts">
/**
 * 应用根组件：路由视图 + 全局错误边界 + Toast 容器 + 全局拖拽打开。
 */
import { onBeforeUnmount, onErrorCaptured, onMounted, ref } from 'vue'
import { RouterView, useRouter } from 'vue-router'
import { useToast } from './composables/useToast'
import { useDocumentStore } from './stores/document'
import { onFileDragDrop } from './services/platformService'

const router = useRouter()
const documentStore = useDocumentStore()
const { state: toastState, dismiss } = useToast()

const fatalError = ref<string | null>(null)
const dragging = ref(false)

// 组件树错误兜底：任何渲染器/页面异常都不允许白屏
onErrorCaptured((err, _instance, info) => {
  console.error('[FeatherView] 组件错误:', info, err)
  const message = err instanceof Error ? err.message : String(err)
  fatalError.value = `页面渲染时发生错误：${message}`
  return false
})

window.addEventListener('error', (e) => {
  console.error('[FeatherView] 全局错误:', e.message)
})
window.addEventListener('unhandledrejection', (e) => {
  console.error('[FeatherView] 未处理的 Promise 拒绝:', e.reason)
})

// 全局拖拽：任意页面拖入文件即打开
let unlistenDrag: (() => void) | undefined
onMounted(() => {
  unlistenDrag = onFileDragDrop((source, kind) => {
    if (kind === 'enter' || kind === 'over') {
      dragging.value = true
    } else if (kind === 'leave') {
      dragging.value = false
    } else if (kind === 'drop') {
      dragging.value = false
      if (source) {
        void documentStore.open(source).then((ok) => {
          if (ok && router.currentRoute.value.name !== 'reader') {
            void router.push('/reader')
          }
        })
      }
    }
  })
})

onBeforeUnmount(() => {
  unlistenDrag?.()
})

function reloadApp(): void {
  fatalError.value = null
  window.location.reload()
}
</script>

<template>
  <div class="app-root">
    <RouterView v-if="!fatalError" />
    <div
      v-else
      class="fatal-error"
    >
      <h2>出现了一点问题</h2>
      <p>{{ fatalError }}</p>
      <button
        class="btn btn-primary"
        type="button"
        @click="reloadApp"
      >
        重新加载
      </button>
    </div>

    <div
      v-if="dragging"
      class="drag-overlay"
    >
      <div class="drag-box">
        松开以打开文件
      </div>
    </div>

    <div class="toast-container">
      <div
        v-for="toast in toastState.toasts"
        :key="toast.id"
        class="toast"
        :class="`toast-${toast.kind}`"
        @click="dismiss(toast.id)"
      >
        <div class="toast-title">
          {{ toast.title }}
        </div>
        <div
          v-if="toast.message"
          class="toast-message"
        >
          {{ toast.message }}
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.app-root {
  height: 100%;
}

.fatal-error {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 14px;
  padding: 40px;
  text-align: center;
}

.fatal-error p {
  color: var(--text-secondary);
  max-width: 520px;
  word-break: break-all;
}

.drag-overlay {
  position: fixed;
  inset: 0;
  background: color-mix(in srgb, var(--accent) 12%, transparent);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 900;
  pointer-events: none;
}

.drag-box {
  border: 2px dashed var(--accent);
  border-radius: var(--radius);
  padding: 28px 48px;
  font-size: 16px;
  color: var(--accent);
  background: var(--bg-panel);
}
</style>
