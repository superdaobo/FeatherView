import { describe, expect, it } from 'vitest'
import { matchRenderer } from '../registry'
import { setupRenderers } from '../index'
import type { DocumentMeta } from '../../types'

function makeMeta(extension: string, extra: Partial<DocumentMeta> = {}): DocumentMeta {
  return {
    source: { id: 't', name: `a.${extension}`, extension, path: `C:/a.${extension}` },
    size: 10,
    isBinary: false,
    ...extra,
  }
}

describe('renderer registry', () => {
  setupRenderers()

  it('matches markdown extensions', () => {
    for (const ext of ['md', 'markdown', 'mdown']) {
      expect(matchRenderer(makeMeta(ext))?.id).toBe('markdown')
    }
  })

  it('matches text, code, json and image extensions', () => {
    expect(matchRenderer(makeMeta('txt'))?.id).toBe('text')
    expect(matchRenderer(makeMeta('log'))?.id).toBe('text')
    expect(matchRenderer(makeMeta('ts'))?.id).toBe('code')
    expect(matchRenderer(makeMeta('rs'))?.id).toBe('code')
    expect(matchRenderer(makeMeta('toml'))?.id).toBe('code')
    expect(matchRenderer(makeMeta('json'))?.id).toBe('json')
    expect(matchRenderer(makeMeta('png'))?.id).toBe('image')
    expect(matchRenderer(makeMeta('svg'))?.id).toBe('image')
  })

  it('falls back to unsupported renderer', () => {
    expect(matchRenderer(makeMeta('docx'))?.id).toBe('unsupported')
    expect(matchRenderer(makeMeta(''))?.id).toBe('unsupported')
  })

  it('matches case-insensitively', () => {
    expect(matchRenderer(makeMeta('MD'))?.id).toBe('markdown')
  })
})
