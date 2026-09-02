import { describe, expect, it } from 'vitest'
import { nextDisplayCode, remainingSeconds, type DayTask, type FocusSession } from './model'

const task = (id: string, displayCode: string, parentId: string | null = null): DayTask => ({
  id, permanentTaskId: `task-${id}`, parentId, displayCode, title: displayCode,
  status: 'not_started', importance: 'important', urgency: 'urgent', plannedStart: null,
  plannedEnd: null, createdAt: '2026-09-02T00:00:00.000Z',
})

describe('每日显示编号', () => {
  it('顶级任务按当天最大编号递增', () => {
    expect(nextDisplayCode([task('a', '#1'), task('b', '#3')], null)).toBe('#4')
  })
  it('子任务只在父任务内部递增', () => {
    expect(nextDisplayCode([task('a', '#8'), task('b', '#8.1', 'a')], 'a')).toBe('#8.2')
  })
  it('拒绝第三级任务', () => {
    expect(() => nextDisplayCode([task('a', '#1'), task('b', '#1.1', 'a')], 'b')).toThrow('最多只允许两级')
  })
})

describe('计时恢复', () => {
  it('依据目标结束时间计算，而不是依赖递减计数', () => {
    const focus: FocusSession = { id: 'f', taskId: 'a', status: 'running', plannedSeconds: 1500, remainingSeconds: 1500, targetEndAt: '2026-09-02T10:25:00.000Z', startedAt: '2026-09-02T10:00:00.000Z' }
    expect(remainingSeconds(focus, Date.parse('2026-09-02T10:10:00.000Z'))).toBe(900)
  })
})
