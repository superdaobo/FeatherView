import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it } from 'vitest'
import { useDocumentSearch } from '../useDocumentSearch'
import { ref } from 'vue'

function mountContainer(html: string) {
  const el = document.createElement('div')
  el.innerHTML = html
  document.body.appendChild(el)
  return ref<HTMLElement | null>(el)
}

describe('useDocumentSearch', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    document.body.innerHTML = ''
  })

  it('counts matches and highlights them', () => {
    const container = mountContainer('<p>hello world hello</p><p>again</p>')
    const search = useDocumentSearch(container)
    search.open('hello')
    expect(search.count.value).toBe(2)
    expect(container.value!.querySelectorAll('mark.search-mark').length).toBe(2)
  })

  it('moves current index with next and prev', () => {
    const container = mountContainer('<p>ab ab ab</p>')
    const search = useDocumentSearch(container)
    search.open('ab')
    expect(search.count.value).toBe(3)
    expect(search.currentIndex.value).toBe(0)

    search.next()
    expect(search.currentIndex.value).toBe(1)
    search.next()
    expect(search.currentIndex.value).toBe(2)
    search.next()
    // 循环
    expect(search.currentIndex.value).toBe(0)
    search.prev()
    expect(search.currentIndex.value).toBe(2)
  })

  it('shows zero count when no result', () => {
    const container = mountContainer('<p>nothing here</p>')
    const search = useDocumentSearch(container)
    search.open('zzz')
    expect(search.count.value).toBe(0)
    expect(container.value!.querySelectorAll('mark').length).toBe(0)
  })

  it('clears marks on close', () => {
    const container = mountContainer('<p>hello</p>')
    const search = useDocumentSearch(container)
    search.open('hello')
    expect(container.value!.querySelectorAll('mark').length).toBe(1)
    search.close()
    expect(container.value!.querySelectorAll('mark').length).toBe(0)
    expect(search.active.value).toBe(false)
  })

  it('matches case-insensitively', () => {
    const container = mountContainer('<p>Hello HELLO</p>')
    const search = useDocumentSearch(container)
    search.open('hello')
    expect(search.count.value).toBe(2)
  })

  it('re-runs search on updated query', () => {
    const container = mountContainer('<p>apple banana</p>')
    const search = useDocumentSearch(container)
    search.open('apple')
    expect(search.count.value).toBe(1)
    search.query.value = 'banana'
    search.runSearch()
    expect(search.count.value).toBe(1)
  })
})
