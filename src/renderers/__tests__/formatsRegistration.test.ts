import { defineAsyncComponent } from 'vue'
import { describe, expect, it } from 'vitest'
import { matchRenderer, registerRenderer } from '../registry'
import type { DocumentMeta } from '../../types'

function makeMeta(extension: string): DocumentMeta {
  return {
    source: { id: 't', name: `a.${extension}`, extension, path: `C:/a.${extension}` },
    size: 10,
    isBinary: false,
  }
}

/**
 * 与主 Agent 合并 src/renderers/index.ts 的注册建议保持一致（见 Agent E 报告 §3）。
 * 本测试独立注册验证扩展名匹配；主 Agent 合并后重复注册同 id 仅覆盖，测试仍绿。
 */
const FORMAT_RENDERERS = [
  {
    id: 'pdf',
    name: 'PDF 阅读器',
    extensions: ['pdf'],
    component: () => import('../pdf/PdfViewer.vue'),
  },
  {
    id: 'csv',
    name: 'CSV 表格阅读器',
    extensions: ['csv', 'tsv'],
    component: () => import('../csv/CsvViewer.vue'),
  },
  {
    id: 'archive',
    name: '压缩包内容列表',
    extensions: ['zip', 'tar', 'gz', 'tgz'],
    component: () => import('../archive/ArchiveViewer.vue'),
  },
]

describe('formats renderer registration', () => {
  for (const def of FORMAT_RENDERERS) {
    registerRenderer({
      id: def.id,
      name: def.name,
      extensions: [...def.extensions],
      canHandle: () => false,
      component: defineAsyncComponent(def.component),
    })
  }

  it('matches pdf extension', () => {
    expect(matchRenderer(makeMeta('pdf'))?.id).toBe('pdf')
    expect(matchRenderer(makeMeta('PDF'))?.id).toBe('pdf')
  })

  it('matches csv and tsv extensions', () => {
    expect(matchRenderer(makeMeta('csv'))?.id).toBe('csv')
    expect(matchRenderer(makeMeta('tsv'))?.id).toBe('csv')
  })

  it('matches archive extensions', () => {
    for (const ext of ['zip', 'tar', 'gz', 'tgz']) {
      expect(matchRenderer(makeMeta(ext))?.id).toBe('archive')
    }
  })
})