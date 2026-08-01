import { describe, expect, it, vi } from 'vitest'
import {
  closeReadSession,
  MAX_SESSION_CHUNK_BYTES,
  MAX_SESSION_TOTAL_BYTES,
  openReadSession,
  readRange,
  readSessionAll,
} from '../largeFileService'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))

describe('largeFileService', () => {
  it('returns null when open_read_session is not implemented (graceful)', async () => {
    invokeMock.mockRejectedValue(new Error('Command open_read_session not found'))
    expect(await openReadSession('C:/big.log')).toBeNull()
  })

  it('returns null when read_range fails', async () => {
    invokeMock.mockRejectedValue(new Error('boom'))
    expect(await readRange('s1', 0, 100)).toBeNull()
  })

  it('reads all chunks until eof and closes the session', async () => {
    const chunks = [
      { bytesRead: 3, nextOffset: 3, eof: false, text: 'abc', encoding: 'utf-8' },
      { bytesRead: 3, nextOffset: 6, eof: true, text: 'def', encoding: 'utf-8' },
    ]
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'open_read_session') {
        return { sessionId: 's1', info: { size: 6, mtime: 1, encoding: 'utf-8', isText: true, suggestedChunkBytes: 3 } }
      }
      if (cmd === 'read_range') return chunks.shift()
      if (cmd === 'close_read_session') return null
      throw new Error(`unknown command: ${cmd}`)
    })

    const session = await openReadSession('C:/big.log')
    expect(session).not.toBeNull()
    expect(session!.info.isText).toBe(true)
    const text = await readSessionAll(session!.sessionId, 3)
    expect(text).toBe('abcdef')
    await closeReadSession(session!.sessionId)
    expect(invokeMock).toHaveBeenCalledWith('close_read_session', { sessionId: 's1' })
  })

  it('clamps chunk size to the 1MB contract limit', async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'open_read_session') {
        return { sessionId: 's1', info: { size: 0, mtime: 1, encoding: 'utf-8', isText: true, suggestedChunkBytes: 1 } }
      }
      if (cmd === 'read_range') {
        return { bytesRead: 0, nextOffset: 0, eof: true, text: '', encoding: 'utf-8' }
      }
      throw new Error(`unknown command: ${cmd}`)
    })
    const session = await openReadSession('C:/big.log')
    await readSessionAll(session!.sessionId, MAX_SESSION_CHUNK_BYTES + 1000)
    expect(invokeMock).toHaveBeenCalledWith('read_range', expect.objectContaining({ length: MAX_SESSION_CHUNK_BYTES }))
  })

  it('throws when the session exceeds the frontend total limit', async () => {
    let offset = 0
    const block = 5 * 1024 * 1024
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'open_read_session') {
        return { sessionId: 's1', info: { size: 0, mtime: 1, encoding: 'utf-8', isText: true, suggestedChunkBytes: block } }
      }
      if (cmd === 'read_range') {
        const res = { bytesRead: block, nextOffset: offset + block, eof: false, text: 'x'.repeat(block), encoding: 'utf-8' }
        offset = res.nextOffset
        return res
      }
      throw new Error(`unknown command: ${cmd}`)
    })
    const session = await openReadSession('C:/big.log')
    await expect(readSessionAll(session!.sessionId, block)).rejects.toThrow('超出前端读取上限')
    expect(offset).toBeGreaterThan(MAX_SESSION_TOTAL_BYTES)
  })
})
