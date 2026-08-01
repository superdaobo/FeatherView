/**
 * 窗口服务：窗口标题等窗口级操作的唯一入口。
 * 组件不得直接调用 window.setTitle。
 */
import { getCurrentWindow } from '@tauri-apps/api/window'
import { isTauri } from './platformService'

export const DEFAULT_WINDOW_TITLE = 'FeatherView / 轻阅'

/** 设置窗口标题（Tauri 环境设置原生标题；浏览器开发模式回退 document.title） */
export function setWindowTitle(title: string): void {
  if (isTauri()) {
    try {
      void getCurrentWindow().setTitle(title)
    } catch {
      // 静默：标题失败不影响功能
    }
  } else {
    document.title = title
  }
}
