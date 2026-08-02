import { describe, expect, it } from 'vitest'
import { fileUrlToPath, fileUrlToSource, isFileUrl } from '../iosAdapter'

describe('isFileUrl', () => {
  it('detects file scheme case-insensitively', () => {
    expect(isFileUrl('file:///var/tmp/a.md')).toBe(true)
    expect(isFileUrl('FILE:///var/tmp/a.md')).toBe(true)
  })

  it('rejects other schemes and bare paths', () => {
    expect(isFileUrl('http://example.com/a.md')).toBe(false)
    expect(isFileUrl('content://com.example/doc/1')).toBe(false)
    expect(isFileUrl('/var/tmp/a.md')).toBe(false)
  })
})

describe('fileUrlToPath', () => {
  it('decodes percent-encoding and strips scheme', () => {
    expect(fileUrlToPath('file:///var/tmp/hello%20world.md')).toBe('/var/tmp/hello world.md')
  })

  it('decodes utf8 sequences', () => {
    expect(fileUrlToPath('file:///var/tmp/%E6%B5%8B%E8%AF%95.md')).toBe('/var/tmp/测试.md')
  })

  it('handles file://localhost/ prefix', () => {
    expect(fileUrlToPath('file://localhost/private/var/tmp/a.md')).toBe('/private/var/tmp/a.md')
  })

  it('handles triple-slash absolute path', () => {
    expect(fileUrlToPath('file:///var/mobile/Containers/Data/Application/ABC/Documents/Inbox/x.md')).toBe(
      '/var/mobile/Containers/Data/Application/ABC/Documents/Inbox/x.md',
    )
  })

  it('returns non-file input unchanged', () => {
    expect(fileUrlToPath('/plain/path.md')).toBe('/plain/path.md')
    expect(fileUrlToPath('content://a/b')).toBe('content://a/b')
  })

  it('tolerates invalid percent sequences', () => {
    expect(fileUrlToPath('file:///var/tmp/%zz.md')).toBe('/var/tmp/%zz.md')
  })
})

describe('fileUrlToSource', () => {
  it('keeps raw uri and decodes name/extension', () => {
    const s = fileUrlToSource('file:///var/tmp/readme%20%E4%B8%AD%E6%96%87.md', 'text/markdown')
    expect(s.uri).toBe('file:///var/tmp/readme%20%E4%B8%AD%E6%96%87.md')
    expect(s.name).toBe('readme 中文.md')
    expect(s.extension).toBe('md')
    expect(s.mimeType).toBe('text/markdown')
    expect(s.id).toBeTruthy()
  })

  it('falls back to encoded name when decoding fails', () => {
    const s = fileUrlToSource('file:///var/tmp/%zz.txt')
    expect(s.name).toBe('%zz.txt')
    expect(s.extension).toBe('txt')
  })
})