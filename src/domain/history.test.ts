import { describe, expect, it } from 'vitest'
import { groupHistoricalTasks, historicalStatusLabel, type HistoricalUnfinishedTask } from './history'

const task = (values: Partial<HistoricalUnfinishedTask> & Pick<HistoricalUnfinishedTask, 'instanceId' | 'title'>): HistoricalUnfinishedTask => ({
  permanentTaskId: values.instanceId,
  parentInstanceId: null,
  workDate: '2026-09-01',
  displayCode: '#1',
  status: 'not_started',
  importance: 'important',
  urgency: 'urgent',
  reschedulable: true,
  blockedReason: null,
  ...values,
})

describe('historical unfinished task groups', () => {
  it('keeps children with a visible parent and promotes an orphan', () => {
    const groups = groupHistoricalTasks([
      task({ instanceId: 'parent', title: '父任务' }),
      task({ instanceId: 'child', title: '子任务', parentInstanceId: 'parent', displayCode: '#1.1' }),
      task({ instanceId: 'orphan', title: '父任务已完成的子任务', parentInstanceId: 'completed-parent', displayCode: '#2.1' }),
    ])
    expect(groups.map(group => group.root.instanceId)).toEqual(['parent', 'orphan'])
    expect(groups[0].children.map(child => child.instanceId)).toEqual(['child'])
  })

  it('blocks a whole group when one remaining member must be planned elsewhere', () => {
    const groups = groupHistoricalTasks([
      task({ instanceId: 'parent', title: '父任务' }),
      task({ instanceId: 'child', title: '重复目标任务', parentInstanceId: 'parent', reschedulable: false, blockedReason: '请在成长中调整日期' }),
    ])
    expect(groups[0].reschedulable).toBe(false)
    expect(groups[0].blockedReason).toBe('请在成长中调整日期')
    expect(historicalStatusLabel('blocked')).toBe('已阻塞')
  })
})
