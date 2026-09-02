import { useCallback, useEffect, useMemo, useState } from 'react'
import { emptyDay, localDate, type EntryType, type Importance, type ReviewLevel, type TaskStatus, type Urgency } from '../domain/model'
import { createGateway } from '../infrastructure/createGateway'

export function useWorklog() {
  const gateway = useMemo(createGateway, [])
  const workDate = useMemo(localDate, [])
  const [day, setDay] = useState(() => emptyDay(workDate))
  const [busy, setBusy] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const run = useCallback(async (operation: () => ReturnType<typeof gateway.getDaySnapshot>) => {
    setBusy(true); setError(null)
    try { const next = await operation(); setDay(next); return next }
    catch (reason) { const message = reason instanceof Error ? reason.message : String(reason); setError(message); throw reason }
    finally { setBusy(false) }
  }, [])

  useEffect(() => {
    const reload = () => { void run(() => gateway.getDaySnapshot(workDate)).catch(() => undefined) }
    reload()
    window.addEventListener('worklog:reload', reload)
    return () => window.removeEventListener('worklog:reload', reload)
  }, [gateway, run, workDate])

  return {
    day, workDate, busy, error, clearError: () => setError(null),
    createTask: (title: string, importance: Importance, urgency: Urgency, parentId: string | null, plannedStart: string | null, plannedEnd: string | null) => run(() => gateway.createTask({ workDate, title, importance, urgency, parentId, plannedStart, plannedEnd })),
    setTaskStatus: (instanceId: string, status: TaskStatus) => run(() => gateway.setTaskStatus(workDate, instanceId, status)),
    addWorkEntry: (content: string, entryType: EntryType, reviewLevel: ReviewLevel, taskId: string | null) => run(() => gateway.addWorkEntry({ workDate, content, entryType, reviewLevel, taskId })),
    startFocus: (taskId: string, plannedSeconds?: number) => run(async () => {
      const seconds = plannedSeconds ?? (await gateway.getTimerSettings()).workMinutes * 60
      return gateway.startFocus(workDate, taskId, seconds)
    }),
    pauseFocus: () => run(() => gateway.pauseFocus(workDate)),
    resumeFocus: () => run(() => gateway.resumeFocus(workDate)),
    switchFocus: (taskId: string) => run(() => gateway.switchFocus(workDate, taskId)),
    completeFocus: (reason: 'elapsed' | 'early_complete' | 'abandoned') => run(() => gateway.completeFocus(workDate, reason)),
    pauseRest: () => run(() => gateway.pauseRest(workDate)),
    resumeRest: () => run(() => gateway.resumeRest(workDate)),
    completeRest: () => run(() => gateway.completeRest(workDate)),
    skipRest: () => run(() => gateway.skipRest(workDate)),
  }
}
