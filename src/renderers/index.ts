/**
 * 渲染器注册入口：应用启动时调用一次。
 * 所有渲染器组件均动态导入（懒加载），不进入首屏 bundle。
 */
import { defineAsyncComponent } from 'vue'
import { registerRenderer } from './registry'

export function setupRenderers(): void {
  registerRenderer({
    id: 'markdown',
    name: 'Markdown 阅读器',
    extensions: ['md', 'markdown', 'mdown'],
    canHandle: () => false,
    component: defineAsyncComponent(() => import('./markdown/MarkdownRenderer.vue')),
  })

  registerRenderer({
    id: 'text',
    name: '纯文本阅读器',
    extensions: ['txt', 'log'],
    canHandle: () => false,
    component: defineAsyncComponent(() => import('./text/TextViewer.vue')),
  })

  registerRenderer({
    id: 'code',
    name: '代码阅读器',
    extensions: [
      'js', 'ts', 'jsx', 'tsx', 'vue', 'css', 'html', 'htm',
      'py', 'rs', 'java', 'c', 'cpp', 'h', 'hpp', 'cs', 'go',
      'sh', 'ps1', 'yaml', 'yml', 'toml', 'xml', 'ini', 'env',
      'sql', 'bat', 'cmd',
    ],
    canHandle: () => false,
    component: defineAsyncComponent(() => import('./code/CodeViewer.vue')),
  })

  registerRenderer({
    id: 'json',
    name: 'JSON 阅读器',
    extensions: ['json'],
    canHandle: () => false,
    component: defineAsyncComponent(() => import('./json/JsonViewer.vue')),
  })

  registerRenderer({
    id: 'image',
    name: '图片查看器',
    extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'svg', 'bmp', 'ico', 'avif'],
    canHandle: () => false,
    component: defineAsyncComponent(() => import('./image/ImageViewer.vue')),
  })

  // 兜底渲染器：永远最后匹配
  registerRenderer({
    id: 'unsupported',
    name: '不支持的类型',
    extensions: [],
    canHandle: () => true,
    component: defineAsyncComponent(() => import('./unsupported/UnsupportedViewer.vue')),
  })
}
