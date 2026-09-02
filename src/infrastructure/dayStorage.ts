import { emptyDay, type DayState } from '../domain/model'

const key = (date: string) => `worklog.day.${date}`

export function loadDay(date: string): DayState {
  const raw = localStorage.getItem(key(date))
  if (!raw) return emptyDay(date)
  try {
    const value = JSON.parse(raw) as DayState
    return value.workDate === date ? { ...value, rest: value.rest ?? null } : emptyDay(date)
  } catch {
    return emptyDay(date)
  }
}

export function saveDay(value: DayState): void {
  localStorage.setItem(key(value.workDate), JSON.stringify(value))
}
