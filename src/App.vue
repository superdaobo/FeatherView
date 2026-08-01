<script setup lang="ts">
/**
 * 应用根组件：路由视图 + 全局错误边界 + Toast 容器 + 全局拖拽/外部文件打开。
 */
import { onBeforeUnmount, onErrorCaptured, onMounted, ref, watch } from 'vue'
import { RouterView, useRouter } from 'vue-router'
import { useToast } from './composables/useToast'
import { useDocumentStore } from './stores/document'
import { useExternalFileOpen } from './composables/useExternalFileOpen'
import { isFilePath, onFileDragDrop } from './services/platformService'
import { DEFAULT_WINDOW_TITLE, setWindowTitle } from './services/windowService'
import type { DocumentSource } from './types'

const router = useRouter()
const documentStore = useDocumentStore()
const { state: toastState, dismiss, show } = useToast()

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

// ---------- 统一打开文件（所有入口汇聚） ----------
async function openFile(source: DocumentSource): Promise<void> {
  const ok = await documentStore.open(source)
  if (ok && router.currentRoute.value.name !== 'reader') {
    await router.push('/reader')
  }
}

// 冷启动参数 + 第二实例参数（单实例插件事件）
useExternalFileOpen(async (source) => {
  await openFile(source)
})

// 全局拖拽：任意页面拖入文件即打开
let unlistenDrag: (() => void) | undefined
onMounted(() => {
  unlistenDrag = onFileDragDrop((sources, kind) => {
    if (kind === 'enter' || kind === 'over') {
      dragging.value = true
    } else if (kind === 'leave') {
      dragging.value = false
    } else if (kind === 'drop') {
      dragging.value = false
      void (async () => {
        if (!sources || sources.length === 0) return
        if (sources.length > 1) {
          show('当前版本一次只能打开一个文件，已打开第一个文件。', { kind: 'info', duration: 4000 })
        }
        // 文件夹提示：仅当拖入的是文件夹时给出明确提示
        const first = sources[0]
        if (first.path && !(await isFilePath(first.path))) {
          show('暂不支持打开文件夹', { kind: 'error', duration: 3000 })
          return
        }
        await openFile(first)
      })()
    }
  })
})

onBeforeUnmount(() => {
  unlistenDrag?.()
})

// 首页默认标题兜底（ReaderPage 内会随文档更新）
setWindowTitle(DEFAULT_WINDOW_TITLE)

// 离开阅读页时恢复默认标题（ReaderPage 内随文档更新标题）
watch(
  () => router.currentRoute.value.name,
  (name) => {
    if (name !== 'reader') setWindowTitle(DEFAULT_WINDOW_TITLE)
  },
)

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
        松开以使用 FeatherView 打开
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
