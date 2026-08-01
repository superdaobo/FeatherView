import { flushPromises, mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { DocumentMeta } from '../../types'

// pdfjs-dist 尚未安装：整体 mock（依赖安装后本文件无需改动）
vi.mock('pdfjs-dist', () => ({
  GlobalWorkerOptions: { workerSrc: '' },
  getDocument: vi.fn(),
}))

import * as pdfjs from 'pdfjs-dist'
import PdfViewer from '../pdf/PdfViewer.vue'

const mockGetDocument = vi.mocked(pdfjs.getDocument)

function makePage() {
  return {
    getViewport: vi.fn(() => ({ width: 600, height: 800 })),
    render: vi.fn(() => ({ promise: Promise.resolve(), cancel: vi.fn() })),
    cleanup: vi.fn(),
  }
}

function makeDoc(numPages: number) {
  return {
    numPages,
    getPage: vi.fn(async () => makePage()),
    destroy: vi.fn(async () => {}),
  }
}

function makeMeta(): DocumentMeta {
  return {
    source: { id: 'pdf-1', name: 'a.pdf', extension: 'pdf', path: 'C:/a.pdf' },
    size: 1024,
    isBinary: true,
  }
}

function mockLoadingTask(doc: unknown, rejectWith?: unknown) {
  const task = {
    promise: rejectWith !== undefined ? Promise.reject(rejectWith) : Promise.resolve(doc),
    destroy: vi.fn(async () => {}),
  }
  mockGetDocument.mockReturnValue(task as never)
  return task
}

beforeEach(() => {
  mockGetDocument.mockReset()
  // jsdom 无 canvas 2d 实现：置空避免渲染路径报错（组件对 null ctx 直接跳过）
  HTMLCanvasElement.prototype.getContext = vi.fn(() => null) as never
})

describe('PdfViewer', () => {
  it('shows loading state then ready toolbar', async () => {
    mockLoadingTask(makeDoc(3))
    const wrapper = mount(PdfViewer, {
      props: { document: makeMeta(), content: '', bytesBase64: 'AQID' },
    })
    await nextTick()
    expect(wrapper.text()).toContain('正在加载')
    await flushPromises()
    expect(wrapper.text()).toContain('上一页')
    expect(wrapper.text()).toContain('/ 3')
    expect((wrapper.find('.pdf-page-input').element as HTMLInputElement).value).toBe('1')
  })

  it('navigates pages with buttons', async () => {
    const doc = makeDoc(3)
    mockLoadingTask(doc)
    const wrapper = mount(PdfViewer, {
      props: { document: makeMeta(), content: '', bytesBase64: 'AQID' },
    })
    await flushPromises()
    await wrapper.findAll('button').find((b) => b.text() === '下一页')!.trigger('click')
    expect((wrapper.find('.pdf-page-input').element as HTMLInputElement).value).toBe('2')
    await wrapper.findAll('button').find((b) => b.text() === '上一页')!.trigger('click')
    expect((wrapper.find('.pdf-page-input').element as HTMLInputElement).value).toBe('1')
  })

  it('jumps to typed page number', async () => {
    const doc = makeDoc(5)
    mockLoadingTask(doc)
    const wrapper = mount(PdfViewer, {
      props: { document: makeMeta(), content: '', bytesBase64: 'AQID' },
    })
    await flushPromises()
    const input = wrapper.find('.pdf-page-input')
    await input.setValue('4')
    await input.trigger('change')
    expect((input.element as HTMLInputElement).value).toBe('4')
    expect(doc.getPage).toHaveBeenCalledWith(4)
  })

  it('clamps page number to document range', async () => {
    mockLoadingTask(makeDoc(5))
    const wrapper = mount(PdfViewer, {
      props: { document: makeMeta(), content: '', bytesBase64: 'AQID' },
    })
    await flushPromises()
    const input = wrapper.find('.pdf-page-input')
    await input.setValue('99')
    await input.trigger('change')
    expect((input.element as HTMLInputElement).value).toBe('5')
  })

  it('zooms in and out', async () => {
    mockLoadingTask(makeDoc(2))
    const wrapper = mount(PdfViewer, {
      props: { document: makeMeta(), content: '', bytesBase64: 'AQID' },
    })
    await flushPromises()
    expect(wrapper.find('.pdf-scale-label').text()).toBe('100%')
    await wrapper.findAll('button').find((b) => b.text() === '放大')!.trigger('click')
    expect(wrapper.find('.pdf-scale-label').text()).toBe('125%')
    await wrapper.findAll('button').find((b) => b.text() === '缩小')!.trigger('click')
    expect(wrapper.find('.pdf-scale-label').text()).toBe('100%')
  })

  it('fits width without crashing in jsdom', async () => {
    mockLoadingTask(makeDoc(2))
    const wrapper = mount(PdfViewer, {
      props: { document: makeMeta(), content: '', bytesBase64: 'AQID' },
    })
    await flushPromises()
    await wrapper.findAll('button').find((b) => b.text() === '适应宽度')!.trigger('click')
    await flushPromises()
    expect(wrapper.find('.pdf-scale-label').exists()).toBe(true)
  })

  it('shows error for corrupted pdf', async () => {
    mockLoadingTask(null, { name: 'InvalidPDFException' })
    const wrapper = mount(PdfViewer, {
      props: { document: makeMeta(), content: '', bytesBase64: 'AQID' },
    })
    await flushPromises()
    expect(wrapper.text()).toContain('损坏')
  })

  it('shows error for password-protected pdf', async () => {
    mockLoadingTask(null, { name: 'PasswordException' })
    const wrapper = mount(PdfViewer, {
      props: { document: makeMeta(), content: '', bytesBase64: 'AQID' },
    })
    await flushPromises()
    expect(wrapper.text()).toContain('密码')
  })

  it('shows error when bytesBase64 is missing', async () => {
    const wrapper = mount(PdfViewer, {
      props: { document: makeMeta(), content: '' },
    })
    await flushPromises()
    expect(wrapper.text()).toContain('未提供 PDF 数据')
  })

  it('destroys document on unmount', async () => {
    const doc = makeDoc(3)
    mockLoadingTask(doc)
    const wrapper = mount(PdfViewer, {
      props: { document: makeMeta(), content: '', bytesBase64: 'AQID' },
    })
    await flushPromises()
    wrapper.unmount()
    expect(doc.destroy).toHaveBeenCalled()
  })
})