import { describe, expect, it } from 'vitest'
import { normalizeFontScale } from './appearance'

describe('界面字号', () => {
  it('限制范围并按 5% 对齐', () => {
    expect(normalizeFontScale(82)).toBe(85)
    expect(normalizeFontScale(113)).toBe(115)
    expect(normalizeFontScale(180)).toBe(130)
  })

  it('无效值回退到标准字号', () => {
    expect(normalizeFontScale(Number.NaN)).toBe(100)
  })
})
