import { id, nextDisplayCode, remainingSeconds, timelineEvent, type DayState, type DayTask, type TaskStatus } from '../domain/model'
import type { CloseDayRequest, CloseDayResult, CreateTaskRequest, EndOfDayPreview, WorkEntryRequest, WorklogGateway } from '../application/gateway'
import { loadDay, saveDay } from './dayStorage'

const commit = (day: DayState) => { saveDay(day); return structuredClone(day) }
const nextDate = (workDate: string) => {
  const [year, month, day] = workDate.split('-').map(Number)
  const next = new Date(Date.UTC(year, month - 1, day + 1))
  return next.toISOString().slice(0, 10)
}
const isCarryCandidate = (status: TaskStatus) => status !== 'completed' && status !== 'cancelled'

const findTask = (day: DayState, taskId: string) => {
  const task = day.tasks.find((item) => item.id === taskId)
  if (!task) throw new Error('任务不存在或已移出今天')
  return task
}

export class BrowserGateway implements WorklogGateway {
  async getDaySnapshot(workDate: string) { return structuredClone(loadDay(workDate)) }

  async createTask(input: CreateTaskRequest) {
    const day = loadDay(input.workDate)
    const parent = input.parentId ? findTask(day, input.parentId) : null
    const displayCode = nextDisplayCode(day.tasks, input.parentId)
    const task: DayTask = {
      id: id(), permanentTaskId: id(), parentId: input.parentId, displayCode, title: input.title,
      status: 'not_started', importance: parent?.importance ?? input.importance,
      urgency: parent?.urgency ?? input.urgency, plannedStart: input.plannedStart,
      plannedEnd: input.plannedEnd, createdAt: new Date().toISOString(),
    }
    return commit({ ...day, tasks: [...day.tasks, task], timeline: [...day.timeline, timelineEvent('task.created', `新增任务${displayCode}：${input.title}`)] })
  }

  async setTaskStatus(workDate: string, instanceId: string, status: TaskStatus) {
    const day = loadDay(workDate)
    const task = findTask(day, instanceId)
    const labels: Record<TaskStatus, string> = { not_started: '恢复', in_progress: '开始', waiting: '等待', blocked: '阻塞', completed: '完成', deferred: '延期', cancelled: '取消' }
    return commit({ ...day, tasks: day.tasks.map((item) => item.id === instanceId ? { ...item, status } : item), timeline: [...day.timeline, timelineEvent(`task.${status}`, `${labels[status]}${task.displayCode}：${task.title}`)] })
  }

  async addWorkEntry(input: WorkEntryRequest) {
    const day = loadDay(input.workDate)
    const task = input.taskId ? findTask(day, input.taskId) : null
    const visibility = input.reviewLevel === 'scratch' ? 'hidden' : input.reviewLevel === 'key' ? 'summary' : 'detail'
    const title = `${task ? `${task.displayCode} · ` : ''}${input.content}`
    return commit({ ...day, timeline: [...day.timeline, timelineEvent('work_entry.created', title, visibility, `${input.entryType} · ${input.reviewLevel}`)] })
  }

  async startFocus(workDate: string, taskId: string, plannedSeconds: number) {
    const day = loadDay(workDate)
    if (day.focus) throw new Error('已有正在进行的专注')
    const task = findTask(day, taskId)
    const now = Date.now()
    return commit({ ...day, tasks: day.tasks.map((item) => item.id === taskId && item.status === 'not_started' ? { ...item, status: 'in_progress' } : item), focus: { id: id(), taskId, status: 'running', plannedSeconds, remainingSeconds: plannedSeconds, targetEndAt: new Date(now + plannedSeconds * 1000).toISOString(), startedAt: new Date(now).toISOString() }, timeline: [...day.timeline, timelineEvent('focus.started', `开始一轮工作，任务内容：${task.displayCode} ${task.title}`)] })
  }

  async pauseFocus(workDate: string) {
    const day = loadDay(workDate)
    if (!day.focus || day.focus.status !== 'running') throw new Error('当前没有可暂停的专注')
    const remaining = remainingSeconds(day.focus)
    return commit({ ...day, focus: { ...day.focus, status: 'paused', remainingSeconds: remaining, targetEndAt: null }, timeline: [...day.timeline, timelineEvent('focus.paused', '暂停本轮工作', 'detail', `剩余${Math.ceil(remaining / 60)}分钟`)] })
  }

  async resumeFocus(workDate: string) {
    const day = loadDay(workDate)
    if (!day.focus || day.focus.status !== 'paused') throw new Error('当前没有已暂停的专注')
    return commit({ ...day, focus: { ...day.focus, status: 'running', targetEndAt: new Date(Date.now() + day.focus.remainingSeconds * 1000).toISOString() }, timeline: [...day.timeline, timelineEvent('focus.resumed', '继续本轮工作', 'detail')] })
  }

  async switchFocus(workDate: string, taskId: string) {
    const day = loadDay(workDate)
    if (!day.focus) throw new Error('当前没有进行中的专注')
    const oldTask = findTask(day, day.focus.taskId)
    const task = findTask(day, taskId)
    return commit({ ...day, focus: { ...day.focus, taskId }, timeline: [...day.timeline, timelineEvent('focus.task_switched', `本轮工作由${oldTask.displayCode}切换至${task.displayCode}`)] })
  }

  async completeFocus(workDate: string, reason: 'elapsed' | 'early_complete' | 'abandoned') {
    const day = loadDay(workDate)
    if (!day.focus) return day
    const actual = Math.max(0, day.focus.plannedSeconds - remainingSeconds(day.focus))
    const minutes = Math.max(1, Math.round(actual / 60))
    const title = reason === 'abandoned' ? `放弃本轮工作，已进行${minutes}分钟` : `完成一轮工作，共${minutes}分钟`
    return commit({ ...day, focus: null, timeline: [...day.timeline, timelineEvent(`focus.${reason}`, title)] })
  }

  async previewEndOfDay(workDate: string): Promise<EndOfDayPreview> {
    const day = loadDay(workDate)
    return {
      workDate,
      nextWorkDate: nextDate(workDate),
      totalCount: day.tasks.length,
      completedCount: day.tasks.filter((task) => task.status === 'completed').length,
      waitingCount: day.tasks.filter((task) => task.status === 'waiting').length,
      blockedCount: day.tasks.filter((task) => task.status === 'blocked').length,
      candidates: day.tasks.filter((task) => isCarryCandidate(task.status)).map((task) => ({
        instanceId: task.id, permanentTaskId: task.permanentTaskId, parentId: task.parentId,
        displayCode: task.displayCode, title: task.title, status: task.status,
        importance: task.importance, urgency: task.urgency,
      })),
      alreadyClosed: day.timeline.some((event) => event.type === 'day.closed'),
    }
  }

  async closeDay(input: CloseDayRequest): Promise<CloseDayResult> {
    const source = loadDay(input.workDate)
    if (source.focus) throw new Error('请先结束或放弃当前专注，再进行日终收尾')
    if (source.timeline.some((event) => event.type === 'day.closed')) throw new Error('今天已经完成日终收尾，不能重复顺延')
    if (nextDate(input.workDate) !== input.nextWorkDate) throw new Error('顺延目标必须是紧接着的下一自然日')
    const eligible = new Set(source.tasks.filter((task) => isCarryCandidate(task.status)).map((task) => task.id))
    const selected = new Set(input.selectedInstanceIds)
    if ([...selected].some((id) => !eligible.has(id))) throw new Error('顺延列表包含已完成、已取消或不存在的事项')

    const next = loadDay(input.nextWorkDate)
    const carriedTasks = [...next.tasks]
    const destination = new Map<string, string>()
    for (const task of source.tasks.filter((item) => selected.has(item.id))) {
      if (carriedTasks.some((item) => item.permanentTaskId === task.permanentTaskId)) throw new Error(`次日已经包含任务${task.displayCode}`)
      const parentId = task.parentId ? destination.get(task.parentId) ?? null : null
      const displayCode = nextDisplayCode(carriedTasks, parentId)
      const status: TaskStatus = task.status === 'waiting' || task.status === 'blocked' ? task.status : 'not_started'
      const carried: DayTask = {
        ...task, id: id(), parentId, displayCode, status,
        plannedStart: null, plannedEnd: null, createdAt: new Date().toISOString(),
      }
      carriedTasks.push(carried)
      destination.set(task.id, carried.id)
    }
    const completed = source.tasks.filter((task) => task.status === 'completed').length
    const carriedCount = selected.size
    const skippedCount = eligible.size - carriedCount
    const sourceTasks = source.tasks.map((task): DayTask => {
      const shouldDefer = selected.has(task.id) && (task.status === 'not_started' || task.status === 'in_progress')
      return shouldDefer ? { ...task, status: 'deferred' } : task
    })
    const sourceDay = {
      ...source, tasks: sourceTasks,
      timeline: [...source.timeline, timelineEvent('day.closed', `日终收尾：完成${completed}项，顺延${carriedCount}项至${input.nextWorkDate}`)],
    }
    const nextDay = {
      ...next, tasks: carriedTasks,
      timeline: carriedCount > 0 ? [...next.timeline, timelineEvent('day.carryover_received', `从${input.workDate}顺延${carriedCount}项任务`, 'detail')] : next.timeline,
    }
    saveDay(sourceDay)
    saveDay(nextDay)
    return { sourceDay: structuredClone(sourceDay), nextDay: structuredClone(nextDay), carriedCount, skippedCount }
  }
}
