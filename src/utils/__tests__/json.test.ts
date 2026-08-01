import { describe, expect, it } from 'vitest'
import { parseJson } from '../json'

describe('parseJson', () => {
  it('parses and formats valid json', () => {
    const result = parseJson('{"a":1,"b":[1,2]}')
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.formatted).toContain('\n')
      expect(result.formatted).toContain('"a": 1')
    }
  })

  it('handles empty input', () => {
    const result = parseJson('   ')
    expect(result.ok).toBe(true)
  })

  it('reports invalid json with message', () => {
    const result = parseJson('{invalid')
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.message.length).toBeGreaterThan(0)
    }
  })

  it('reports trailing garbage', () => {
    const result = parseJson('{"a":1} extra')
    expect(result.ok).toBe(false)
  })
})
