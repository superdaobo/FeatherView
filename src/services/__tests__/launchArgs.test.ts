import { describe, expect, it } from 'vitest'
import { parseLaunchArgs, stripQuotes } from '../launchArgs'

function fakeExists(existing: string[]): (p: string) => Promise<boolean> {
  const set = new Set(existing.map((p) => p.toLowerCase()))
  return async (p: string) => set.has(p.toLowerCase())
}

const EXISTING = [
  'C:\\docs\\普通文档.md',
  'C:\\docs\\带 空 格 的文档.md',
  'C:\\docs\\测试.多个.点.markdown',
  'C:\\docs\\中文日志.log',
  'C:\\docs\\settings.json',
  'C:\\docs\\config.yaml',
]

describe('stripQuotes', () => {
  it('strips paired quotes', () => {
    expect(stripQuotes('"C:\\a\\b.md"')).toBe('C:\\a\\b.md')
    expect(stripQuotes("'C:\\a\\b.md'")).toBe('C:\\a\\b.md')
  })

  it('keeps unpaired quotes', () => {
    expect(stripQuotes('"C:\\a\\b.md')).toBe('"C:\\a\\b.md')
  })

  it('handles nested quotes', () => {
    expect(stripQuotes('""C:\\a.md""')).toBe('C:\\a.md')
  })
})

describe('parseLaunchArgs', () => {
  const exists = fakeExists(EXISTING)

  it('parses a single normal path', async () => {
    const result = await parseLaunchArgs(['featherview.exe', 'C:\\docs\\普通文档.md'], exists)
    expect(result.files).toEqual(['C:\\docs\\普通文档.md'])
    expect(result.flags).toBe(0)
  })

  it('parses chinese path', async () => {
    const result = await parseLaunchArgs(['featherview.exe', 'C:\\docs\\中文日志.log'], exists)
    expect(result.files).toEqual(['C:\\docs\\中文日志.log'])
  })

  it('parses path with spaces', async () => {
    const result = await parseLaunchArgs(['featherview.exe', 'C:\\docs\\带 空 格 的文档.md'], exists)
    expect(result.files).toEqual(['C:\\docs\\带 空 格 的文档.md'])
  })

  it('parses path with multiple dots', async () => {
    const result = await parseLaunchArgs(['featherview.exe', 'C:\\docs\\测试.多个.点.markdown'], exists)
    expect(result.files).toEqual(['C:\\docs\\测试.多个.点.markdown'])
  })

  it('parses quoted path', async () => {
    const result = await parseLaunchArgs(['featherview.exe', '"C:\\docs\\settings.json"'], exists)
    expect(result.files).toEqual(['C:\\docs\\settings.json'])
  })

  it('ignores non-existent path', async () => {
    const result = await parseLaunchArgs(['featherview.exe', 'C:\\docs\\missing.md'], exists)
    expect(result.files).toEqual([])
    expect(result.ignored).toBe(1)
  })

  it('ignores folder path', async () => {
    const result = await parseLaunchArgs(['featherview.exe', 'C:\\docs'], exists)
    expect(result.files).toEqual([])
    expect(result.ignored).toBe(1)
  })

  it('ignores url', async () => {
    const result = await parseLaunchArgs(['featherview.exe', 'https://example.com/a.md'], exists)
    expect(result.files).toEqual([])
    expect(result.ignored).toBe(1)
  })

  it('returns multiple files in order', async () => {
    const result = await parseLaunchArgs(
      ['featherview.exe', 'C:\\docs\\a.md', 'C:\\docs\\b.md', 'C:\\docs\\c.md'],
      fakeExists(['C:\\docs\\a.md', 'C:\\docs\\c.md']),
    )
    expect(result.files).toEqual(['C:\\docs\\a.md', 'C:\\docs\\c.md'])
  })

  it('handles flag mixed with paths', async () => {
    const result = await parseLaunchArgs(
      ['featherview.exe', '--verbose', 'C:\\docs\\settings.json', '/open'],
      exists,
    )
    expect(result.files).toEqual(['C:\\docs\\settings.json'])
    expect(result.flags).toBe(2)
  })

  it('handles empty args', async () => {
    const result = await parseLaunchArgs([], exists)
    expect(result.files).toEqual([])
    expect(result.flags).toBe(0)
  })

  it('handles only flags', async () => {
    const result = await parseLaunchArgs(['featherview.exe', '-h', '--help'], exists)
    expect(result.files).toEqual([])
    expect(result.flags).toBe(2)
  })

  it('dedupes case-insensitively on windows', async () => {
    const result = await parseLaunchArgs(
      ['featherview.exe', 'C:\\Docs\\Settings.json', 'c:\\docs\\settings.json'],
      exists,
    )
    expect(result.files).toEqual(['C:\\Docs\\Settings.json'])
  })

  it('ignores unsupported extension but still opens (unsupported renderer handles it)', async () => {
    const result = await parseLaunchArgs(
      ['featherview.exe', 'C:\\docs\\sample.xyz'],
      fakeExists(['C:\\docs\\sample.xyz']),
    )
    expect(result.files).toEqual(['C:\\docs\\sample.xyz'])
  })

  it('never throws on invalid exists callback', async () => {
    const result = await parseLaunchArgs(['featherview.exe', 'C:\\a.md'], async () => {
      throw new Error('boom')
    })
    expect(result.files).toEqual([])
    expect(result.ignored).toBe(1)
  })
})
