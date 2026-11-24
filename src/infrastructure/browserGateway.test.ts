import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { BrowserGateway } from './browserGateway'
import { loadStoredDay } from './dayStorage'
import { LONG_TERM_FOCUS_PREFIX } from '../domain/longTerm'
import { localDate } from '../domain/model'

const gateway = new BrowserGateway()
const workDate = '2026-11-23'
const taskInput = { workDate, title: '校核', importance: 'important', urgency: 'urgent', plannedStart: null, plannedEnd: null } as const

beforeEach(() => {
  const storage: Record<string, string> = {}
  Object.defineProperties(storage, {
    getItem: { value: (key: string) => storage[key] ?? null },
    setItem: { value: (key: string, value: string) => { storage[key] = value } },
  })
  vi.stubGlobal('localStorage', storage)
})
afterEach(() => vi.unstubAllGlobals())

describe('long-term work integration', () => {
  it('keeps long-term activity once in the daily timeline after later writes', async () => {
    const workDate = localDate()
    const [parent] = await gateway.saveLongTermWork({ id: null, parentId: null, title: '设备选型', workload: null, completed: false })
    const [, child] = await gateway.saveLongTermWork({ id: null, parentId: parent.id, title: '收集规格', workload: 'light', completed: false })
    await gateway.saveLongTermWork({ ...child, completed: true })
    await gateway.saveLongTermWork({ ...child, completed: true })
    await gateway.saveLongTermWork({ ...parent, completed: true })
    await gateway.saveLongTermWork({ ...child, workload: 'large', completed: false })
    const day = await gateway.createTask({ ...taskInput, workDate, parentId: null })
    await gateway.moveTaskToLongTerm({ workDate, instanceId: day.tasks[0].id, parentId: null, title: '另一项计划', workload: null })
    const { timeline } = await gateway.getDaySnapshot(workDate)
    expect(timeline.filter((event) => event.type.startsWith('long_term.')).map((event) => event.title)).toEqual([
      '新增长期任务：设备选型',
      '新增长期子事项：设备选型 / 收集规格，工作量：轻松（<1 h）',
      '完成长期子事项：设备选型 / 收集规格',
      '完成长期任务：设备选型',
      '重开长期子事项：设备选型 / 收集规格，工作量：轻松（<1 h） → 较大（1–2 h）',
    ])
  })

  it('moves a daily group with the original IDs, estimates and records', async () => {
    const { tasks: [parent] } = await gateway.createTask({ ...taskInput, parentId: null })
    const day = await gateway.createTask({ ...taskInput, parentId: parent.id, workload: 'light' })
    const child = day.tasks[1]
    expect(day.timeline.at(-1)?.title).toContain('工作量：轻松（<1 h）')
    const edited = await gateway.updateTask({ ...taskInput, instanceId: child.id, importance: null, urgency: null, workload: 'large' })
    expect(edited.timeline.at(-1)?.title).toContain('轻松（<1 h） → 较大（1–2 h）')
    const moved = await gateway.moveTaskToLongTerm({ workDate, instanceId: parent.id, parentId: null, title: '长期校核', workload: null })
    expect(moved.tasks).toEqual([])
    expect(moved.timeline).toHaveLength(4)
    expect(loadStoredDay(workDate).tasks).toHaveLength(2)
    expect(await gateway.listLongTermWork()).toEqual([
      { id: parent.permanentTaskId, parentId: null, title: '长期校核', workload: null, completed: false },
      { id: child.permanentTaskId, parentId: parent.permanentTaskId, title: '校核', workload: 'large', completed: false },
    ])
    const next = await gateway.createTask({ ...taskInput, parentId: null })
    expect(next.tasks[0].displayCode).toBe('#2')
    const read = await gateway.getDaySnapshot(workDate)
    expect(read.timeline.filter((event) => event.type === 'task.moved_to_long_term')).toHaveLength(1)
    expect(loadStoredDay(workDate).tasks).toHaveLength(3)
  })

  it('starts and switches long-term focus with correctly associated thoughts', async () => {
    const [{ id }] = await gateway.saveLongTermWork({ id: null, parentId: null, title: '设备选型', workload: null, completed: false })
    const target = LONG_TERM_FOCUS_PREFIX + id
    const start = await gateway.startFocus(workDate, target, 1500)
    expect(start.tasks).toEqual([])
    expect(start.focus?.longTermId).toBe(id)
    const record = await gateway.addWorkEntry({ workDate, taskId: target, content: '整理规格', entryType: 'idea', reviewLevel: 'normal' })
    expect(record.timeline.at(-1)?.title).toBe('长期 · 设备选型：整理规格')
    const daily = await gateway.createTask({ ...taskInput, parentId: null })
    const switched = await gateway.switchFocus(workDate, daily.tasks[0].id)
    expect(switched.focus?.longTermId).toBeNull()
    const back = await gateway.switchFocus(workDate, target)
    expect(back.focus?.longTermId).toBe(id)
  })
})
