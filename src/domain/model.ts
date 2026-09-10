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

export interface DailySchedule {
  id: string
  title: string
  plannedStart: string
  plannedEnd: string
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

export interface TimelineGroup {
  id: string
  occurredAt: string
  events: TimelineEvent[]
}

export interface FocusSession {
  id: string
  taskId: string
  status: 'running' | 'paused'
  plannedSeconds: number
  remainingSeconds: number
  targetEndAt: string | null
  startedAt: string
  timerMode: 'countdown' | 'count_up'
  elapsedSeconds: number
  runningStartedAt: string | null
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
  schedules: DailySchedule[]
  timeline: TimelineEvent[]
  focus: FocusSession | null
  rest: RestSession | null
}

export const id = () => crypto.randomUUID()

export function localDate(): string {
  const now = new Date()
  return [now.getFullYear(), String(now.getMonth() + 1).padStart(2, '0'), String(now.getDate()).padStart(2, '0')].join('-')
}

export const emptyDay = (workDate = localDate()): DayState => ({ workDate, tasks: [], schedules: [], timeline: [], focus: null, rest: null })

export function validScheduleRange(start: string, end: string): boolean {
  const pattern = /^(?:[01]\d|2[0-3]):[0-5]\d$/
  return pattern.test(start) && pattern.test(end) && start < end
}

export function schedulesByTime(schedules: DailySchedule[]): DailySchedule[] {
  return [...schedules].sort((left, right) => left.plannedStart.localeCompare(right.plannedStart) || left.createdAt.localeCompare(right.createdAt))
}

export function groupTimelineEvents(events: TimelineEvent[], thresholdSeconds = 120): TimelineGroup[] {
  const groups: TimelineGroup[] = []
  for (const event of events) {
    const previousGroup = groups.at(-1)
    const previousEvent = previousGroup?.events.at(-1)
    const currentTime = Date.parse(event.occurredAt)
    const previousTime = previousEvent ? Date.parse(previousEvent.occurredAt) : Number.NaN
    const distance = currentTime - previousTime
    if (previousGroup && Number.isFinite(distance) && distance >= 0 && distance < thresholdSeconds * 1000) {
      previousGroup.events.push(event)
    } else {
      groups.push({ id: event.id, occurredAt: event.occurredAt, events: [event] })
    }
  }
  return groups
}

export function resolveWorkEntryTask(tasks: DayTask[], focusTaskId: string | null, selectedTaskId: string | null): DayTask | null {
  return tasks.find((task) => task.id === focusTaskId)
    ?? tasks.find((task) => !task.parentId && task.id === selectedTaskId)
    ?? tasks.find((task) => !task.parentId && task.status !== 'completed')
    ?? null
}

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

export function incompleteFirst<T extends Pick<DayTask, 'status'>>(tasks: T[]): T[] {
  return [...tasks].sort((left, right) => Number(left.status === 'completed') - Number(right.status === 'completed'))
}

export function remainingSeconds(focus: Pick<FocusSession, 'status' | 'remainingSeconds' | 'targetEndAt'>, now = Date.now()): number {
  if (focus.status === 'paused' || !focus.targetEndAt) return focus.remainingSeconds
  return Math.max(0, Math.ceil((new Date(focus.targetEndAt).getTime() - now) / 1000))
}

export function elapsedFocusSeconds(focus: Pick<FocusSession, 'status' | 'elapsedSeconds' | 'runningStartedAt'>, now = Date.now()): number {
  const running = focus.status === 'running' && focus.runningStartedAt ? Math.max(0, Math.floor((now - new Date(focus.runningStartedAt).getTime()) / 1000)) : 0
  return Math.max(0, focus.elapsedSeconds + running)
}

export function timelineEvent(type: string, title: string, visibility: TimelineEvent['visibility'] = 'summary', detail: string | null = null): TimelineEvent {
  return { id: id(), type, occurredAt: new Date().toISOString(), title, detail, visibility }
}
