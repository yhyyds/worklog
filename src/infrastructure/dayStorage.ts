import { emptyDay, type DayState } from '../domain/model'
import { loadLongTermStorage, longTermEvents } from './longTermStorage'

const key = (date: string) => `worklog.day.${date}`

export function loadStoredDay(date: string): DayState {
  const raw = localStorage.getItem(key(date))
  if (!raw) return emptyDay(date)
  try {
    const value = JSON.parse(raw) as DayState
    return value.workDate === date ? { ...value, schedules: value.schedules ?? [], rest: value.rest ?? null } : emptyDay(date)
  } catch {
    return emptyDay(date)
  }
}

export function loadDay(date: string): DayState {
  const stored = loadStoredDay(date)
  const storage = loadLongTermStorage()
  const movedIds = new Set(storage.transfers.filter((transfer) => transfer.workDate <= date).flatMap((transfer) => transfer.taskIds))
  const events = longTermEvents(storage, date)
  return {
    ...stored,
    tasks: stored.tasks.filter((task) => !movedIds.has(task.permanentTaskId)),
    timeline: events.length ? [...stored.timeline, ...events].sort((a, b) => a.occurredAt.localeCompare(b.occurredAt)) : stored.timeline,
  }
}

export function saveDay(value: DayState): void {
  const storage = loadLongTermStorage()
  const movedIds = new Set(storage.transfers.filter((transfer) => transfer.workDate <= value.workDate).flatMap((transfer) => transfer.taskIds))
  const eventIds = new Set(longTermEvents(storage, value.workDate).map((event) => event.id))
  const archived = loadStoredDay(value.workDate).tasks.filter((task) => movedIds.has(task.permanentTaskId))
  localStorage.setItem(key(value.workDate), JSON.stringify({
    ...value,
    tasks: [...value.tasks.filter((task) => !movedIds.has(task.permanentTaskId)), ...archived],
    timeline: value.timeline.filter((event) => !eventIds.has(event.id)),
  }))
}
