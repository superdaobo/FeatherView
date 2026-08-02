import { beforeEach, describe, expect, it, vi } from 'vitest'
import { openDocument } from '../documentService'
import type { DocumentSource } from '../../types'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))

function makeSource(name: string, path: string, extension: string): DocumentSource {
  return { id: `id-${name}`, name, extension, path }
}

describe('documentService', () => {
  beforeEach(() => {
    invokeMock.mockReset()
    ;(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {}
  })

  it('returns a skeleton for large text files (no read_file call)', async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'file_metadata') return { size: 11 * 1024 * 1024, modifiedAt: undefined, isFile: true }
      throw new Error('read_file should not be called for large text')
    })
    const { meta, result } = await openDocument(makeSource('big.log', 'C:/big.log', 'log'))
    expect(result.content).toBe('')
    expect(meta.size).toBe(11 * 1024 * 1024)
    expect(invokeMock).not.toHaveBeenCalledWith('read_file', expect.anything())
  })

  it('keeps the normal read path for small files', async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'file_metadata') return { size: 100, modifiedAt: undefined, isFile: true }
      if (cmd === 'read_file') return { content: 'hello', size: 100, isBinary: false }
      throw new Error(`unknown command: ${cmd}`)
    })
    const { result } = await openDocument(makeSource('a.md', 'C:/a.md', 'md'))
    expect(result.content).toBe('hello')
    expect(invokeMock).toHaveBeenCalledWith('read_file', { path: 'C:/a.md' })
  })

  it('does not skeleton large binary files (image extension)', async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'file_metadata') return { size: 20 * 1024 * 1024, modifiedAt: undefined, isFile: true }
      if (cmd === 'read_file') {
        return { bytesBase64: 'AAAA', size: 20 * 1024 * 1024, isBinary: true }
      }
      throw new Error(`unknown command: ${cmd}`)
    })
    const { result } = await openDocument(makeSource('photo.png', 'C:/photo.png', 'png'))
    expect(result.bytesBase64).toBe('AAAA')
  })
})