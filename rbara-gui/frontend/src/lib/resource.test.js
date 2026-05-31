import { describe, it, expect } from 'vitest'
import { sizeExceedsLimit } from './resource.js'

const MB = 1024 // kilobytes per megabyte

describe('sizeExceedsLimit (hard block gate)', () => {
  describe('blocks oversized files', () => {
    it('blocks when size exceeds the limit', () => {
      expect(sizeExceedsLimit(201 * MB, 200)).toBe(true)
      expect(sizeExceedsLimit(650 * MB, 200)).toBe(true)
    })

    it('allows when size is under the limit', () => {
      expect(sizeExceedsLimit(150 * MB, 200)).toBe(false)
    })

    it('allows exactly at the limit (strict greater-than)', () => {
      expect(sizeExceedsLimit(200 * MB, 200)).toBe(false)
    })
  })

  describe('limit disabled (0 = no limit)', () => {
    it('does not block any known size when the limit is 0', () => {
      expect(sizeExceedsLimit(150 * MB, 0)).toBe(false)
      expect(sizeExceedsLimit(650 * MB, 0)).toBe(false)
      expect(sizeExceedsLimit(5000 * MB, 0)).toBe(false)
    })

    it('treats a negative limit as disabled', () => {
      expect(sizeExceedsLimit(650 * MB, -1)).toBe(false)
    })

    it('treats a missing / non-numeric limit as disabled', () => {
      expect(sizeExceedsLimit(650 * MB, undefined)).toBe(false)
      expect(sizeExceedsLimit(650 * MB, null)).toBe(false)
      expect(sizeExceedsLimit(650 * MB, '200')).toBe(false)
    })
  })

  describe('unknown / invalid size fails safe (blocks)', () => {
    it('blocks when size is null, 0, NaN, or non-finite, even with a disabled limit', () => {
      expect(sizeExceedsLimit(null, 200)).toBe(true)
      expect(sizeExceedsLimit(0, 200)).toBe(true)
      expect(sizeExceedsLimit(NaN, 200)).toBe(true)
      expect(sizeExceedsLimit(Infinity, 200)).toBe(true)
      expect(sizeExceedsLimit('123', 200)).toBe(true) // non-number size
      expect(sizeExceedsLimit(null, 0)).toBe(true) // unknown size still blocks
    })
  })
})
