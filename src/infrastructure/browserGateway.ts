import { id, nextDisplayCode, remainingSeconds, timelineEvent, type DayState, type DayTask, type TaskStatus } from '../domain/model'
import type { CreateTaskRequest, WorkEntryRequest, WorklogGateway } from '../application/gateway'
import { loadDay, saveDay } from './dayStorage'

const commit = (day: DayState) => { saveDay(day); return structuredClone(day) }
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
}
