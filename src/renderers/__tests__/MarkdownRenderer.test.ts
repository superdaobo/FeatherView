import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it } from 'vitest'
import MarkdownRenderer from '../markdown/MarkdownRenderer.vue'
import type { DocumentMeta } from '../../types'

function makeMeta(): DocumentMeta {
  return {
    source: { id: '1', name: 'test.md', extension: 'md', path: 'C:/docs/test.md' },
    size: 100,
    isBinary: false,
  }
}

describe('MarkdownRenderer', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('renders markdown content as html', () => {
    const wrapper = mount(MarkdownRenderer, {
      props: { document: makeMeta(), content: '# 标题\n\n正文内容' },
    })
    expect(wrapper.find('h1').text()).toBe('标题')
    expect(wrapper.text()).toContain('正文内容')
    expect(wrapper.attributes('data-renderer')).toBe('markdown')
  })

  it('does not execute raw html', () => {
    const wrapper = mount(MarkdownRenderer, {
      props: { document: makeMeta(), content: '<script>window.__x=1</script>' },
    })
    expect(wrapper.find('script').exists()).toBe(false)
  })

  it('emits toc from headings', async () => {
    const wrapper = mount(MarkdownRenderer, {
      props: { document: makeMeta(), content: '# One\n\n## Two' },
    })
    const tocEvents = wrapper.emitted('toc')
    expect(tocEvents).toBeTruthy()
    const toc = tocEvents![0][0] as Array<{ text: string; level: number }>
    expect(toc.map((t) => t.text)).toEqual(['One', 'Two'])
    expect(toc[0].level).toBe(1)
  })

  it('highlights code blocks', () => {
    const wrapper = mount(MarkdownRenderer, {
      props: { document: makeMeta(), content: '```ts\nconst x: number = 1\n```' },
    })
    expect(wrapper.find('pre code.language-ts').exists()).toBe(true)
    // highlight.js 输出内部高亮 span
    expect(wrapper.find('pre .hljs-keyword').exists()).toBe(true)
  })

  it('adds copy buttons to code blocks', () => {
    const wrapper = mount(MarkdownRenderer, {
      props: { document: makeMeta(), content: '```js\nconsole.log(1)\n```' },
    })
    expect(wrapper.find('pre .copy-code-btn').exists()).toBe(true)
  })

  it('applies content width from settings', async () => {
    const { useSettingsStore } = await import('../../stores/settings')
    const store = useSettingsStore()
    store.settings.contentWidth = 'narrow'
    store.settings.fontFamily = 'serif'

    const wrapper = mount(MarkdownRenderer, {
      props: { document: makeMeta(), content: 'text' },
    })
    expect(wrapper.attributes('data-width')).toBe('narrow')
    expect(wrapper.attributes('data-font')).toBe('serif')
  })
})
