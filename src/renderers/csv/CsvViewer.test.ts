import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import CsvViewer from '../csv/CsvViewer.vue'
import type { DocumentMeta } from '../../types'

function makeMeta(ext = 'csv'): DocumentMeta {
  return {
    source: { id: '1', name: `a.${ext}`, extension: ext, path: `C:/a.${ext}` },
    size: 100,
    isBinary: false,
  }
}

describe('CsvViewer', () => {
  it('renders header row and data rows', () => {
    const wrapper = mount(CsvViewer, {
      props: { document: makeMeta(), content: 'name,age\nAlice,30\nBob,25' },
    })
    const headers = wrapper.findAll('th')
    expect(headers.length).toBe(2)
    expect(headers[0].text()).toBe('name')
    expect(wrapper.findAll('tbody tr').length).toBe(2)
  })

  it('shows row/column counts and detected delimiter', () => {
    const wrapper = mount(CsvViewer, {
      props: { document: makeMeta(), content: 'a;b;c\n1;2;3' },
    })
    expect(wrapper.text()).toContain('1 行 × 3 列')
    expect(wrapper.text()).toContain('分隔符 ;')
  })

  it('keeps quoted fields with delimiters intact', () => {
    const wrapper = mount(CsvViewer, {
      props: { document: makeMeta(), content: 'a,b\n"x,y",2' },
    })
    expect(wrapper.findAll('tbody td')[0].text()).toBe('x,y')
  })

  it('shows truncation notice for large files', () => {
    const lines = ['h1,h2', ...Array.from({ length: 5001 }, () => '1,2')]
    const wrapper = mount(CsvViewer, {
      props: { document: makeMeta(), content: lines.join('\n') },
    })
    expect(wrapper.text()).toContain('仅显示前 5000 行')
  })

  it('shows empty state for empty content', () => {
    const wrapper = mount(CsvViewer, {
      props: { document: makeMeta(), content: '' },
    })
    expect(wrapper.text()).toContain('空文件')
  })

  it('shows issues for ragged rows', () => {
    const wrapper = mount(CsvViewer, {
      props: { document: makeMeta(), content: 'a,b,c\n1,2' },
    })
    expect(wrapper.text()).toContain('字段数')
  })
})