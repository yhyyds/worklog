import { describe, expect, it } from 'vitest'
import { incompleteFirst, nextDisplayCode, remainingSeconds, schedulesByTime, validScheduleRange, type DayTask, type FocusSession } from './model'

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
    const focus: FocusSession = { id: 'f', taskId: 'a', status: 'running', plannedSeconds: 1500, remainingSeconds: 1500, targetEndAt: '2026-09-02T10:25:00.000Z', startedAt: '2026-09-02T10:00:00.000Z', timerMode: 'countdown', elapsedSeconds: 0, runningStartedAt: null }
    expect(remainingSeconds(focus, Date.parse('2026-09-02T10:10:00.000Z'))).toBe(900)
  })
})

describe('任务视觉排序', () => {
  it('将已完成事项稳定移动到当前栏目的底部', () => {
    const completed = { ...task('done', '#1'), status: 'completed' as const }
    const pending = task('pending', '#2')
    const active = { ...task('active', '#3'), status: 'in_progress' as const }
    expect(incompleteFirst([completed, pending, active]).map((item) => item.id)).toEqual(['pending', 'active', 'done'])
  })
})

describe('固定安排', () => {
  it('要求有效且递增的时间段', () => {
    expect(validScheduleRange('14:00', '15:30')).toBe(true)
    expect(validScheduleRange('15:30', '14:00')).toBe(false)
    expect(validScheduleRange('24:00', '25:00')).toBe(false)
  })

  it('按开始时间稳定排序', () => {
    const items = [
      { id: 'b', title: '复盘', plannedStart: '17:00', plannedEnd: '17:30', createdAt: '2026-09-07T01:00:00Z' },
      { id: 'a', title: '会议', plannedStart: '14:00', plannedEnd: '15:00', createdAt: '2026-09-07T00:00:00Z' },
    ]
    expect(schedulesByTime(items).map((item) => item.id)).toEqual(['a', 'b'])
  })
})
