import { describe, expect, it } from 'vitest'
import { extractToc, renderMarkdown } from '../markdown/markdown'

describe('renderMarkdown security', () => {
  it('renders common markdown constructs', () => {
    const html = renderMarkdown('# 标题\n\n**加粗** *斜体* ~~删除线~~\n\n> 引用\n\n- 列表项\n\n```ts\nconst a = 1\n```')
    expect(html).toContain('<h1')
    expect(html).toContain('<strong>加粗</strong>')
    expect(html).toContain('<em>斜体</em>')
    expect(html).toContain('<s>删除线</s>')
    expect(html).toContain('<blockquote>')
    expect(html).toContain('<li>列表项</li>')
    expect(html).toContain('hljs')
  })

  it('renders task lists', () => {
    const html = renderMarkdown('- [x] 已完成\n- [ ] 未完成')
    expect(html).toContain('task-list-item')
    expect(html).toContain('checked')
  })

  it('renders tables', () => {
    const html = renderMarkdown('| a | b |\n|---|---|\n| 1 | 2 |')
    expect(html).toContain('<table>')
    expect(html).toContain('<td>1</td>')
  })

  it('strips scripts from raw html when html is disabled', () => {
    const html = renderMarkdown('<script>alert(1)</script>\n\n<script src="x"></script>')
    // html 关闭时原始标签被转义为纯文本（&lt;script&gt;），不存在可执行的 script 元素
    expect(html).not.toContain('<script')
    expect(html).toContain('&lt;script&gt;')
  })

  it('sanitizes dangerous attributes', () => {
    const html = renderMarkdown('[x](javascript:alert(1))')
    // markdown-it validateLink 拒绝 javascript: 协议，原样输出为纯文本
    expect(html).not.toContain('href=')
    expect(html).toContain('[x](javascript:alert(1))')
  })

  it('adds anchor ids to headings', () => {
    const html = renderMarkdown('## 第一章 简介')
    expect(html).toContain('id=')
    expect(html).toContain('第一章')
  })

  it('handles cjk slugify', () => {
    const html = renderMarkdown('## 你好世界')
    expect(html).toContain('id=')
  })
})

describe('extractToc', () => {
  it('extracts h1-h3 with ids', () => {
    const html = renderMarkdown('# A\n\n## B\n\n### C\n\n#### D')
    const toc = extractToc(html)
    expect(toc.map((t) => t.text)).toEqual(['A', 'B', 'C'])
    expect(toc.map((t) => t.level)).toEqual([1, 2, 3])
    expect(toc.every((t) => t.id.length > 0)).toBe(true)
  })

  it('returns empty for no headings', () => {
    expect(extractToc('<p>plain</p>')).toEqual([])
  })
})
