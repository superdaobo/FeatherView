/**
 * Markdown 渲染管线：
 * 原始 Markdown → markdown-it → DOMPurify → 安全 HTML → 阅读视图
 *
 * 安全要点：
 * - html 默认关闭，禁止原始 HTML（防止 script 注入）
 * - DOMPurify 二次清理
 * - 外部链接拦截并交给系统浏览器
 * - 本地相对图片基于文件目录解析，经 Rust 读取后以 data URL 渲染
 */
import MarkdownIt from 'markdown-it'
import anchor from 'markdown-it-anchor'
import taskLists from 'markdown-it-task-lists'
import DOMPurify from 'dompurify'
import hljs from 'highlight.js/lib/core'

import javascript from 'highlight.js/lib/languages/javascript'
import typescript from 'highlight.js/lib/languages/typescript'
import xml from 'highlight.js/lib/languages/xml'
import css from 'highlight.js/lib/languages/css'
import python from 'highlight.js/lib/languages/python'
import rust from 'highlight.js/lib/languages/rust'
import java from 'highlight.js/lib/languages/java'
import c from 'highlight.js/lib/languages/c'
import cpp from 'highlight.js/lib/languages/cpp'
import csharp from 'highlight.js/lib/languages/csharp'
import go from 'highlight.js/lib/languages/go'
import bash from 'highlight.js/lib/languages/bash'
import powershell from 'highlight.js/lib/languages/powershell'
import json from 'highlight.js/lib/languages/json'
import yaml from 'highlight.js/lib/languages/yaml'
import ini from 'highlight.js/lib/languages/ini'
import sql from 'highlight.js/lib/languages/sql'
import markdown from 'highlight.js/lib/languages/markdown'

hljs.registerLanguage('javascript', javascript)
hljs.registerLanguage('typescript', typescript)
hljs.registerLanguage('xml', xml)
hljs.registerLanguage('css', css)
hljs.registerLanguage('python', python)
hljs.registerLanguage('rust', rust)
hljs.registerLanguage('java', java)
hljs.registerLanguage('c', c)
hljs.registerLanguage('cpp', cpp)
hljs.registerLanguage('csharp', csharp)
hljs.registerLanguage('go', go)
hljs.registerLanguage('bash', bash)
hljs.registerLanguage('powershell', powershell)
hljs.registerLanguage('json', json)
hljs.registerLanguage('yaml', yaml)
hljs.registerLanguage('ini', ini)
hljs.registerLanguage('sql', sql)
hljs.registerLanguage('markdown', markdown)

export const hljsInstance = hljs

/** 常见语言别名 → highlight.js 注册名 */
const LANG_ALIASES: Record<string, string> = {
  ts: 'typescript',
  js: 'javascript',
  jsx: 'javascript',
  mjs: 'javascript',
  sh: 'bash',
  shell: 'bash',
  py: 'python',
  rb: 'ruby',
  'c++': 'cpp',
  hpp: 'cpp',
  h: 'c',
  html: 'xml',
  htm: 'xml',
  yml: 'yaml',
  ps1: 'powershell',
  console: 'bash',
}

function resolveLang(lang: string): string {
  return LANG_ALIASES[lang] ?? lang
}

const md = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: false,
  breaks: false,
  highlight(code: string, lang: string): string {
    const resolved = resolveLang(lang)
    if (resolved && hljs.getLanguage(resolved)) {
      try {
        return hljs.highlight(code, { language: resolved, ignoreIllegals: true }).value
      } catch {
        // 高亮失败退回转义
      }
    }
    return md.utils.escapeHtml(code)
  },
})

md.use(anchor, {
  slugify: (s: string) =>
    encodeURIComponent(
      s
        .trim()
        .toLowerCase()
        .replace(/[^\w\u4e00-\u9fa5]+/g, '-')
        .replace(/^-+|-+$/g, ''),
    ),
  permalink: false,
})
md.use(taskLists, { enabled: true, label: true })

/** 渲染 Markdown 为安全 HTML */
export function renderMarkdown(source: string): string {
  const rawHtml = md.render(source)
  return DOMPurify.sanitize(rawHtml, {
    USE_PROFILES: { html: true },
    ADD_ATTR: ['target', 'rel'],
  })
}

/** 从渲染后的 HTML 中提取标题目录（h1-h3） */
export function extractToc(html: string): Array<{ level: number; text: string; id: string }> {
  const doc = new DOMParser().parseFromString(html, 'text/html')
  const toc: Array<{ level: number; text: string; id: string }> = []
  doc.querySelectorAll('h1, h2, h3').forEach((heading) => {
    const level = Number(heading.tagName.slice(1))
    const id = heading.getAttribute('id') ?? ''
    toc.push({ level, text: heading.textContent?.trim() ?? '', id })
  })
  return toc
}

export default md
