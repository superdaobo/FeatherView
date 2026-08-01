import { describe, expect, it } from 'vitest'
import { MAX_CSV_ROWS, detectDelimiter, parseCsv } from './csvParser'

describe('detectDelimiter', () => {
  it('detects comma', () => {
    expect(detectDelimiter('a,b,c\n1,2,3')).toBe(',')
  })

  it('detects semicolon', () => {
    expect(detectDelimiter('a;b;c\n1;2;3')).toBe(';')
  })

  it('detects tab', () => {
    expect(detectDelimiter('a\tb\tc\n1\t2\t3')).toBe('\t')
  })

  it('ignores delimiters inside quoted fields', () => {
    expect(detectDelimiter('"a,b;c",d')).toBe(',')
  })

  it('defaults to comma on tie', () => {
    expect(detectDelimiter('a,b;c')).toBe(',')
  })

  it('defaults to comma when no candidate found', () => {
    expect(detectDelimiter('只有中文')).toBe(',')
  })
})

describe('parseCsv', () => {
  it('parses basic rows with header', () => {
    const result = parseCsv('name,age\nAlice,30\nBob,25')
    expect(result.header).toEqual(['name', 'age'])
    expect(result.rows).toEqual([
      ['Alice', '30'],
      ['Bob', '25'],
    ])
    expect(result.delimiter).toBe(',')
    expect(result.estimatedTotalRows).toBe(3)
    expect(result.truncated).toBe(false)
  })

  it('parses quoted fields with escaped quotes', () => {
    const result = parseCsv('a,b,c\n1,"x,y",2\n3,"say ""hi""",4')
    expect(result.rows[0]).toEqual(['1', 'x,y', '2'])
    expect(result.rows[1]).toEqual(['3', 'say "hi"', '4'])
  })

  it('keeps newlines inside quoted fields as one row', () => {
    const result = parseCsv('h1,h2\n"line1\nline2",x')
    expect(result.header).toEqual(['h1', 'h2'])
    expect(result.rows).toEqual([['line1\nline2', 'x']])
    expect(result.estimatedTotalRows).toBe(2)
  })

  it('strips BOM', () => {
    const result = parseCsv('\uFEFFa,b\n1,2')
    expect(result.header).toEqual(['a', 'b'])
  })

  it('handles empty file', () => {
    const result = parseCsv('')
    expect(result.header).toEqual([])
    expect(result.rows).toEqual([])
    expect(result.estimatedTotalRows).toBe(0)
    expect(result.issues).toEqual([])
  })

  it('tolerates unclosed quote at end of file', () => {
    const result = parseCsv('a,b\n1,"unclosed')
    expect(result.rows[0]).toEqual(['1', 'unclosed'])
    expect(result.issues.some((issue) => issue.includes('未闭合'))).toBe(true)
  })

  it('reports rows with mismatched field count', () => {
    const result = parseCsv('a,b,c\n1,2\n3,4,5')
    expect(result.rows).toEqual([
      ['1', '2'],
      ['3', '4', '5'],
    ])
    expect(result.issues.some((issue) => issue.includes('字段数'))).toBe(true)
  })

  it('handles CRLF line endings', () => {
    const result = parseCsv('a,b\r\n1,2\r\n3,4')
    expect(result.header).toEqual(['a', 'b'])
    expect(result.rows).toEqual([
      ['1', '2'],
      ['3', '4'],
    ])
  })

  it('truncates beyond MAX_CSV_ROWS data rows', () => {
    const lines = ['h1,h2', ...Array.from({ length: MAX_CSV_ROWS + 5 }, () => '1,2')]
    const result = parseCsv(lines.join('\n'))
    expect(result.truncated).toBe(true)
    expect(result.rows.length).toBe(MAX_CSV_ROWS)
    expect(result.estimatedTotalRows).toBeGreaterThanOrEqual(MAX_CSV_ROWS + 5)
  })
})