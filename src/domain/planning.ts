export interface GoalOccurrence { id: string; date: string; status: string; taskId: string }
export interface GoalAction { id: string; title: string; actionKind: 'one_off' | 'repeating'; required: boolean; targetCount: number; completedCount: number; manualCompletedCount: number; importance: string; urgency: string; tracked: boolean; occurrences: GoalOccurrence[] }
export function dateRange(start: string, end: string, weekdaysOnly = false): string[] {
  const dates: string[] = []
  const first = new Date(`${start}T12:00:00`), last = new Date(`${end}T12:00:00`)
  if (!Number.isFinite(+first) || !Number.isFinite(+last) || first > last) return dates
  for (const day = first; day <= last && dates.length < 3660; day.setDate(day.getDate() + 1)) {
    if (!weekdaysOnly || (day.getDay() !== 0 && day.getDay() !== 6)) dates.push([day.getFullYear(), String(day.getMonth() + 1).padStart(2, '0'), String(day.getDate()).padStart(2, '0')].join('-'))
  }
  return dates
}
