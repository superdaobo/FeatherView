/**
 * Vitest 测试环境初始化。
 * jsdom 缺失的浏览器 API 在此补齐。
 */
import { beforeEach, vi } from 'vitest'

// matchMedia（settings store 使用）
if (!window.matchMedia) {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  })
}

// localStorage（jsdom 已有，但清理遗留数据）
beforeEach(() => {
  localStorage.clear()
  document.documentElement.dataset.theme = 'light'
})

// ResizeObserver 占位（若组件用到）
if (!('ResizeObserver' in window)) {
  class ResizeObserverMock {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  // @ts-expect-error 测试占位
  window.ResizeObserver = ResizeObserverMock
}

// scrollIntoView 占位（jsdom 未实现）
if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {}
}
