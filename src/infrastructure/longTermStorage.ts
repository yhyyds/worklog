import type { LongTermItem } from '../domain/longTerm'
import type { TimelineEvent } from '../domain/model'

interface LongTermStorage {
  items: LongTermItem[]
  transfers: { workDate: string; taskIds: string[]; event: TimelineEvent }[]
  activity?: { workDate: string; event: TimelineEvent }[]
}

const key = 'worklog.longTermWork.v1'

export function loadLongTermStorage(): LongTermStorage {
  const value = JSON.parse(localStorage.getItem(key) ?? '[]') as LongTermStorage | LongTermItem[]
  return Array.isArray(value) ? { items: value, transfers: [] } : value
}

export function saveLongTermStorage(value: LongTermStorage) {
  localStorage.setItem(key, JSON.stringify(value))
}

export function longTermEvents(storage: LongTermStorage, date: string) {
  return [...storage.transfers, ...(storage.activity ?? [])]
    .filter((entry) => entry.workDate === date)
    .map((entry) => entry.event)
}
