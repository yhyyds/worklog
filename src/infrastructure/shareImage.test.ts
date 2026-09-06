import { afterEach, expect, it, vi } from 'vitest'
import { renderOverviewPng } from './shareImage'
import type { Counts, Overview } from '../domain/sharing'
afterEach(() => vi.unstubAllGlobals())
it('exports the sanitized categories and numbers without adding placeholder item names', async () => {
  const written: string[] = []
  const context = { fillStyle: '', textBaseline: '', font: '', fillRect: vi.fn(), fillText: (value: string) => written.push(value), measureText: (s: string) => ({ width: s.length * 20 }) }
  const canvas = { width: 0, height: 0, getContext: () => context, toBlob: (cb: (b: Blob) => void) => cb(new Blob(['png'], { type: 'image/png' })) }
  vi.stubGlobal('document', { fonts: { ready: Promise.resolve() }, createElement: () => canvas })
  const counts: Counts = { planned: 5, completed: 4, focusMinutes: 90, habitDone: 4, habitReviewed: 5, habitPending: 1, habitBreaks: 1, habitBestStreak: 3, goalRequired: 2, goalDone: 1, goalOptional: 1, goalOptionalDone: 0 }
  const r: Overview = { weekStart: '2026-09-07', weekEnd: '2026-09-13', through: '2026-09-08', sharing: true, counts, categories: [{ name: '生活', color: '#449966', counts }], daily: [{ date: '2026-09-07', counts }], history: [], names: [], quote: { id: 'q', text: '千里之行，始于足下。', author: '老子', source: '《道德经》' } }
  const blob = await renderOverviewPng(r)
  expect(blob.type).toBe('image/png'); expect(canvas.height).toBeGreaterThan(500)
  expect(written).toContain('生活'); expect(written.join(' ')).toContain('80%')
  expect(written.join(' ')).not.toContain('隐藏打卡'); expect(written).not.toContain('事项记录')
})
