import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import ArchiveViewer from '../archive/ArchiveViewer.vue'
import type { DocumentMeta } from '../../types'

function makeMeta(ext = 'zip'): DocumentMeta {
  return {
    source: { id: '1', name: `a.${ext}`, extension: ext, path: `C:/a.${ext}` },
    size: 1024,
    isBinary: true,
  }
}

describe('ArchiveViewer', () => {
  it('shows degraded notice when backend not implemented', () => {
    const wrapper = mount(ArchiveViewer, {
      props: { document: makeMeta(), content: '' },
    })
    expect(wrapper.text()).toContain('暂不可用')
  })

  it('renders archive entries with dirs first', () => {
    const wrapper = mount(ArchiveViewer, {
      props: {
        document: makeMeta(),
        content: '',
        archive: {
          format: 'zip',
          entries: [
            { path: 'b.txt', isDir: false, size: 2048 },
            { path: 'dir/', isDir: true, size: 0 },
            { path: 'a.txt', isDir: false, size: 1024 },
          ],
          totalUncompressedSize: 3072,
          truncated: false,
        },
      },
    })
    const names = wrapper.findAll('.archive-name').map((n) => n.text())
    expect(names).toEqual(['dir', 'a.txt', 'b.txt'])
    expect(wrapper.text()).toContain('ZIP')
    expect(wrapper.text()).toContain('3 个条目')
    expect(wrapper.text()).toContain('3.0 KB')
  })

  it('caps visible entries and shows truncation hints', () => {
    const entries = Array.from({ length: 600 }, (_, i) => ({
      path: `f${i}.txt`,
      isDir: false,
      size: 10,
    }))
    const wrapper = mount(ArchiveViewer, {
      props: {
        document: makeMeta(),
        content: '',
        archive: {
          format: 'tar',
          entries,
          totalUncompressedSize: 6000,
          truncated: true,
        },
      },
    })
    expect(wrapper.findAll('.archive-entry').length).toBe(500)
    expect(wrapper.text()).toContain('仅显示前 500 条')
    expect(wrapper.text()).toContain('条目过多')
  })

  it('shows empty state for empty archive', () => {
    const wrapper = mount(ArchiveViewer, {
      props: {
        document: makeMeta(),
        content: '',
        archive: { format: 'zip', entries: [], totalUncompressedSize: 0, truncated: false },
      },
    })
    expect(wrapper.text()).toContain('没有条目')
  })
})