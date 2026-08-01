import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it } from 'vitest'
import TextViewer from '../text/TextViewer.vue'
import type { DocumentMeta } from '../../types'

function makeMeta(ext = 'txt'): DocumentMeta {
  return {
    source: { id: '1', name: `a.${ext}`, extension: ext, path: `C:/a.${ext}` },
    size: 100,
    isBinary: false,
  }
}

describe('TextViewer', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('renders text lines', () => {
    const wrapper = mount(TextViewer, {
      props: { document: makeMeta(), content: '第一行\n第二行\n第三行' },
    })
    expect(wrapper.findAll('.text-line').length).toBe(3)
  })

  it('toggles line numbers from settings', async () => {
    const { useSettingsStore } = await import('../../stores/settings')
    const store = useSettingsStore()
    store.settings.showLineNumbers = true

    const wrapper = mount(TextViewer, {
      props: { document: makeMeta(), content: 'a\nb' },
    })
    expect(wrapper.findAll('.line-num').length).toBe(2)

    // 设置变化后样式更新（响应式生效）
    store.settings.showLineNumbers = false
    await wrapper.vm.$nextTick()
    expect(wrapper.findAll('.line-num').length).toBe(0)
  })

  it('toggles word wrap class from settings', async () => {
    const { useSettingsStore } = await import('../../stores/settings')
    const store = useSettingsStore()
    store.settings.wordWrap = true

    const wrapper = mount(TextViewer, {
      props: { document: makeMeta(), content: 'hello' },
    })
    expect(wrapper.find('.text-content').classes()).toContain('wrap-on')

    store.settings.wordWrap = false
    await wrapper.vm.$nextTick()
    expect(wrapper.find('.text-content').classes()).not.toContain('wrap-on')
  })

  it('applies monospace font class from settings', async () => {
    const { useSettingsStore } = await import('../../stores/settings')
    const store = useSettingsStore()
    store.settings.fontFamily = 'monospace'

    const wrapper = mount(TextViewer, {
      props: { document: makeMeta(), content: 'x' },
    })
    expect(wrapper.find('.text-content').classes()).toContain('mono')
  })

  it('truncates very large content with notice', () => {
    const big = 'x'.repeat(2_000_001)
    const wrapper = mount(TextViewer, {
      props: { document: makeMeta(), content: big },
    })
    expect(wrapper.text()).toContain('文件较大')
  })
})
