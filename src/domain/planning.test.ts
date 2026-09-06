import { describe, expect, it } from 'vitest'
import { dateRange } from './planning'
import { canRollDay } from './dayRollover'
import { goalAward, goalRate, type Counts } from './sharing'
describe('1.1 planning and report rules', () => {
  it('refreshes the date after midnight without interrupting an active timer or write', () => {
    expect(canRollDay('2026-09-06', '2026-09-07', false, false)).toBe(true)
    expect(canRollDay('2026-09-06', '2026-09-07', true, false)).toBe(false)
    expect(canRollDay('2026-09-06', '2026-09-07', false, true)).toBe(false)
    expect(canRollDay('2026-09-07', '2026-09-07', false, false)).toBe(false)
  })
  it('schedules inclusive local dates and weekdays across month boundaries', () => {
    expect(dateRange('2026-09-30', '2026-10-02')).toEqual(['2026-09-30', '2026-10-01', '2026-10-02'])
    expect(dateRange('2026-09-04', '2026-09-07', true)).toEqual(['2026-09-04', '2026-09-07'])
    expect(dateRange('2026-09-08', '2026-09-07')).toEqual([])
  })
  it('optional work can exceed 100%, but cannot replace required work for an award', () => {
    const c = { goalRequired: 5, goalDone: 4, goalOptional: 2, goalOptionalDone: 2 } as Counts
    expect(goalRate(c)).toBe(120); expect(goalAward(c)).toBe('')
    expect(goalAward({ ...c, goalDone: 5 })).toBe('金杯')
    expect(goalAward({ ...c, goalDone: 5, goalOptionalDone: 1 })).toBe('银杯')
    expect(goalRate({ ...c, goalRequired: 0 })).toBeNull()
  })
})
