/**
 * 文档内搜索：基于容器 DOM 文本节点的高亮搜索。
 * 适用于所有渲染器（Markdown 渲染后 DOM、文本/代码内容）。
 */
import { ref, type Ref } from 'vue'

const MAX_SEARCH_CHARS = 2_000_000

export function useDocumentSearch(container: Ref<HTMLElement | null>) {
  const query = ref('')
  const active = ref(false)
  const count = ref(0)
  const currentIndex = ref(0)

  let marks: HTMLElement[] = []

  function clearMarks(): void {
    for (const mark of marks) {
      const parent = mark.parentNode
      if (!parent) continue
      const text = document.createTextNode(mark.textContent ?? '')
      parent.replaceChild(text, mark)
      parent.normalize()
    }
    marks = []
    count.value = 0
    currentIndex.value = 0
  }

  function highlightCurrent(): void {
    marks.forEach((m, i) => {
      m.classList.toggle('current', i === currentIndex.value)
    })
  }

  /** 在容器中执行搜索（内容变化后需重新调用） */
  function runSearch(): void {
    clearMarks()
    const q = query.value.trim()
    const root = container.value
    if (!q || !root) {
      count.value = 0
      return
    }

    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        return node.textContent && node.textContent.length > 0
          ? NodeFilter.FILTER_ACCEPT
          : NodeFilter.FILTER_REJECT
      },
    })

    const needle = q.toLowerCase()
    let scanned = 0

    const textNodes: Text[] = []
    let node: Node | null = walker.nextNode()
    while (node) {
      textNodes.push(node as Text)
      scanned += (node.textContent?.length ?? 0)
      if (scanned > MAX_SEARCH_CHARS) break
      node = walker.nextNode()
    }

    for (const textNode of textNodes) {
      const text = textNode.textContent ?? ''
      const lower = text.toLowerCase()
      // 收集本节点内所有匹配偏移
      const offsets: number[] = []
      let idx = 0
      while (idx <= text.length - needle.length) {
        const found = lower.indexOf(needle, idx)
        if (found < 0) break
        offsets.push(found)
        idx = found + needle.length
      }
      // 从后往前包裹，避免 splitText 影响后续偏移
      for (let i = offsets.length - 1; i >= 0; i--) {
        const start = offsets[i]
        const range = document.createRange()
        range.setStart(textNode, start)
        range.setEnd(textNode, start + needle.length)
        const mark = document.createElement('mark')
        mark.className = 'search-mark'
        range.surroundContents(mark)
        marks.unshift(mark)
      }
    }

    count.value = marks.length
    currentIndex.value = marks.length > 0 ? 0 : 0
    highlightCurrent()
  }

  function goTo(index: number): void {
    if (marks.length === 0) return
    currentIndex.value = (index + marks.length) % marks.length
    highlightCurrent()
    const mark = marks[currentIndex.value]
    mark.scrollIntoView({ behavior: 'smooth', block: 'center' })
  }

  function next(): void {
    goTo(currentIndex.value + 1)
  }

  function prev(): void {
    goTo(currentIndex.value - 1)
  }

  function open(q = ''): void {
    query.value = q
    active.value = true
    runSearch()
  }

  function close(): void {
    active.value = false
    query.value = ''
    clearMarks()
  }

  return { query, active, count, currentIndex, open, close, runSearch, next, prev }
}
