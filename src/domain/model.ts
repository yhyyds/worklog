export type Importance = 'important' | 'secondary'
export type Urgency = 'urgent' | 'relaxed'
export type TaskStatus = 'not_started' | 'in_progress' | 'waiting' | 'blocked' | 'completed' | 'deferred' | 'cancelled'
export type EntryType = 'progress' | 'idea' | 'decision' | 'blocker' | 'result'
export type ReviewLevel = 'key' | 'normal' | 'scratch'

export interface DayTask {
  id: string
  permanentTaskId: string
  parentId: string | null
  displayCode: string
  title: string
  status: TaskStatus
  importance: Importance
  urgency: Urgency
  plannedStart: string | null
  plannedEnd: string | null
  createdAt: string
}

export interface TimelineEvent {
  id: string
  type: string
  occurredAt: string
  title: string
  detail: string | null
  visibility: 'summary' | 'detail' | 'hidden'
}

export interface FocusSession {
  id: string
  taskId: string
  status: 'running' | 'paused'
  plannedSeconds: number
  remainingSeconds: number
  targetEndAt: string | null
  startedAt: string
}

export interface RestSession {
  id: string
  restKind: 'short' | 'long'
  status: 'running' | 'paused'
  plannedSeconds: number
  remainingSeconds: number
  targetEndAt: string | null
  startedAt: string
}

export interface DayState {
  workDate: string
  tasks: DayTask[]
  timeline: TimelineEvent[]
  focus: FocusSession | null
  rest: RestSession | null
}

export const id = () => crypto.randomUUID()

export function localDate(): string {
  const now = new Date()
  return [now.getFullYear(), String(now.getMonth() + 1).padStart(2, '0'), String(now.getDate()).padStart(2, '0')].join('-')
}

export const emptyDay = (workDate = localDate()): DayState => ({ workDate, tasks: [], timeline: [], focus: null, rest: null })

export function nextDisplayCode(tasks: DayTask[], parentId: string | null): string {
  if (!parentId) {
    const largest = tasks.filter((task) => !task.parentId).reduce((max, task) => Math.max(max, Number(task.displayCode.slice(1)) || 0), 0)
    return `#${largest + 1}`
  }
  const parent = tasks.find((task) => task.id === parentId)
  if (!parent) throw new Error('父任务不存在')
  if (parent.parentId) throw new Error('任务最多只允许两级')
  const prefix = `${parent.displayCode}.`
  const largest = tasks.filter((task) => task.parentId === parentId).reduce((max, task) => Math.max(max, Number(task.displayCode.slice(prefix.length)) || 0), 0)
  return `${prefix}${largest + 1}`
}

export function remainingSeconds(focus: Pick<FocusSession, 'status' | 'remainingSeconds' | 'targetEndAt'>, now = Date.now()): number {
  if (focus.status === 'paused' || !focus.targetEndAt) return focus.remainingSeconds
  return Math.max(0, Math.ceil((new Date(focus.targetEndAt).getTime() - now) / 1000))
}

export function timelineEvent(type: string, title: string, visibility: TimelineEvent['visibility'] = 'summary', detail: string | null = null): TimelineEvent {
  return { id: id(), type, occurredAt: new Date().toISOString(), title, detail, visibility }
}
