// Keep an overnight focus/rest on its original day until it has ended.
export function canRollDay(current: string, today: string, busy: boolean, activeTimer: boolean): boolean {
  return current !== today && !busy && !activeTimer
}
