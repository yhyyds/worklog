import { describe, expect, it } from 'vitest'
import { minutesText, rateText, streakText, trend } from './report'
import { wrapLines } from '../infrastructure/reportImage'

describe('weekly report display', () => {
  it('distinguishes no data from zero completion', () => {
    expect(rateText(0, 0)).toBe('—')
    expect(rateText(0, 5)).toBe('0%')
    expect(rateText(4, 5)).toBe('80%')
  })
  it('does not show an upward arrow for an unchanged week', () => {
    expect(trend(0)).toBe('与上周持平')
    expect(trend(null)).toBe('上周无记录')
    expect(trend(-25)).toBe('比上周减少 25%')
  })
  it('does not present an unreviewed streak as broken', () => {
    expect(streakText({ currentStreak: null, streakThrough: '2026-09-06' })).toBe('待回顾')
    expect(streakText({ currentStreak: 0, streakThrough: '2026-09-06' })).toBe('0 天')
    expect(streakText({ currentStreak: 12, streakThrough: '2026-09-06' })).toBe('12 天')
  })
  it('formats focus durations without decimals', () => {
    expect(minutesText(150)).toBe('2小时30分')
    expect(minutesText(60)).toBe('1小时')
    expect(minutesText(0)).toBe('0分钟')
  })
  it('wraps long titles without discarding characters', () => {
    const measure = { measureText: (value: string) => ({ width: value.length * 10 } as TextMetrics) }
    const title = '一项很长的计划：学习常见电器元件及符号'
    const lines = wrapLines(measure, title, 50)
    expect(lines.join('')).toBe(title)
    expect(lines.every(line => line.length <= 5)).toBe(true)
    expect(wrapLines(measure, '第一行\n第二行', 100)).toEqual(['第一行', '第二行'])
  })
})
