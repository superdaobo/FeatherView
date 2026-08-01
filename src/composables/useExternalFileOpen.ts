/**
 * 外部文件打开统一入口：
 * - 冷启动参数（文件关联/双击/命令行）
 * - 第二实例参数（single-instance 插件转发事件）
 * 多文件只取第一个并提示一次；同一文件短时间内去重；监听器随组件卸载清理。
 */
import { onBeforeUnmount, onMounted, ref } from 'vue'
import type { DocumentSource } from '../types'
import { initializeExternalOpenListener, resolveInitialDocument } from '../services/platformService'
import { useToast } from './useToast'

const DEDUPE_WINDOW_MS = 2000

export function useExternalFileOpen(
  openFile: (source: DocumentSource) => Promise<void>,
): { handleSources: (sources: DocumentSource[]) => Promise<void> } {
  const { show } = useToast()
  const opening = ref(false)
  let lastKey = ''
  let lastHandledAt = 0
  let unlisten: (() => void) | undefined

  async function handleSources(sources: DocumentSource[]): Promise<void> {
    if (sources.length === 0) return
    const key = sources[0].path ?? sources[0].uri ?? ''
    // 同一文件短时间内的重复事件只处理一次
    if (key && key === lastKey && Date.now() - lastHandledAt < DEDUPE_WINDOW_MS) return
    if (opening.value) return
    opening.value = true
    try {
      if (sources.length > 1) {
        show('当前版本一次只能打开一个文件，已打开第一个文件。', { kind: 'info', duration: 4000 })
      }
      lastKey = key
      lastHandledAt = Date.now()
      await openFile(sources[0])
    } finally {
      opening.value = false
    }
  }

  onMounted(() => {
    void (async () => {
      // 1. 冷启动：启动参数中的文件
      try {
        const initial = await resolveInitialDocument()
        if (initial) {
          await handleSources([initial])
        }
      } catch {
        // 静默：不允许异常影响启动
      }
      // 2. 第二实例：监听外部打开事件
      try {
        unlisten = await initializeExternalOpenListener((sources) => {
          void handleSources(sources)
        })
      } catch {
        // 监听失败不影响主流程
      }
    })()
  })

  onBeforeUnmount(() => {
    unlisten?.()
  })

  return { handleSources }
}
