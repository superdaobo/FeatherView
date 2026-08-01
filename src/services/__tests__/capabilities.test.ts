import { describe, expect, it } from 'vitest'
import { getPlatformCapabilities, listDirectory, pickFolder } from '../capabilities'

describe('capabilities (temporary platform capabilities)', () => {
  it('reports desktop capabilities in browser dev mode', () => {
    const caps = getPlatformCapabilities()
    expect(caps.folderBrowsing).toBe(true)
    expect(caps.fileManagement).toBe(true)
    expect(caps.nativePath).toBe(true)
  })

  it('pickFolder returns null without tauri', async () => {
    expect(await pickFolder()).toBeNull()
  })

  it('listDirectory returns null without tauri (graceful fallback)', async () => {
    expect(await listDirectory('C:/some/dir')).toBeNull()
  })
})