import { describe, expect, it } from 'vitest'
import {
  countLines,
  countWords,
  formatDate,
  formatFileSize,
  getDirName,
  getExtension,
  getFileName,
} from '../index'

describe('formatFileSize', () => {
  it('formats bytes', () => {
    expect(formatFileSize(0)).toBe('0 B')
    expect(formatFileSize(512)).toBe('512 B')
  })

  it('formats KB and MB', () => {
    expect(formatFileSize(1024)).toBe('1.0 KB')
    expect(formatFileSize(2048)).toBe('2.0 KB')
    expect(formatFileSize(5 * 1024 * 1024)).toBe('5.0 MB')
  })

  it('formats GB', () => {
    expect(formatFileSize(2 * 1024 * 1024 * 1024)).toBe('2.00 GB')
  })

  it('handles undefined', () => {
    expect(formatFileSize(undefined)).toBe('—')
  })
})

describe('getExtension', () => {
  it('extracts lowercase extension', () => {
    expect(getExtension('C:\\docs\\file.MD')).toBe('md')
    expect(getExtension('/home/user/note.markdown')).toBe('markdown')
    expect(getExtension('archive.tar.gz')).toBe('gz')
  })

  it('returns empty for no extension', () => {
    expect(getExtension('README')).toBe('')
    expect(getExtension('.gitignore')).toBe('')
    expect(getExtension('dir/')).toBe('')
  })
})

describe('getFileName / getDirName', () => {
  it('handles windows and posix separators', () => {
    expect(getFileName('C:\\a\\b\\c.md')).toBe('c.md')
    expect(getFileName('/a/b/c.md')).toBe('c.md')
    expect(getDirName('C:\\a\\b\\c.md')).toBe('C:\\a\\b')
    expect(getDirName('/a/b/c.md')).toBe('/a/b')
  })
})

describe('countLines / countWords', () => {
  it('counts lines with mixed endings', () => {
    expect(countLines('a\nb\r\nc\rd')).toBe(4)
    expect(countLines('')).toBe(0)
    expect(countLines('single')).toBe(1)
  })

  it('counts cjk and latin words', () => {
    expect(countWords('你好世界')).toBe(4)
    expect(countWords('hello world')).toBe(2)
    expect(countWords('你好 hello')).toBe(3)
  })
})

describe('formatDate', () => {
  it('formats timestamp', () => {
    const ts = new Date(2024, 0, 15, 9, 5).getTime()
    expect(formatDate(ts)).toBe('2024-01-15 09:05')
  })

  it('handles undefined', () => {
    expect(formatDate(undefined)).toBe('—')
  })
})
