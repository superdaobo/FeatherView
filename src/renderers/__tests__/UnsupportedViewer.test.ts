import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import UnsupportedViewer from '../unsupported/UnsupportedViewer.vue'
import { setupRenderers } from '../index'
import type { DocumentMeta } from '../../types'

setupRenderers()

function makeMeta(ext: string): DocumentMeta {
  return {
    source: { id: '1', name: `a.${ext}`, extension: ext, path: `C:/a.${ext}` },
    size: 10,
    isBinary: false,
  }
}

describe('UnsupportedViewer', () => {
  it('shows friendly message with extension', () => {
    const wrapper = mount(UnsupportedViewer, {
      props: { document: makeMeta('docx'), content: '' },
    })
    expect(wrapper.text()).toContain('暂不支持 .docx 文件')
    expect(wrapper.text()).toContain('Markdown')
  })

  it('emits reopen and home actions', async () => {
    const wrapper = mount(UnsupportedViewer, {
      props: { document: makeMeta('pdf'), content: '' },
    })
    const buttons = wrapper.findAll('button')
    expect(buttons.length).toBeGreaterThanOrEqual(2)

    await wrapper.findAll('button')[0].trigger('click')
    await wrapper.findAll('button')[1].trigger('click')
    expect(wrapper.emitted('reopen')).toBeTruthy()
    expect(wrapper.emitted('home')).toBeTruthy()
  })
})
