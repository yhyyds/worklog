export interface HistoricalUnfinishedTask {
  instanceId: string
  permanentTaskId: string
  parentInstanceId: string | null
  workDate: string
  displayCode: string
  title: string
  status: string
  importance: string
  urgency: string
  reschedulable: boolean
  blockedReason: string | null
}

export interface HistoricalTaskGroup {
  root: HistoricalUnfinishedTask
  children: HistoricalUnfinishedTask[]
  reschedulable: boolean
  blockedReason: string | null
}

export function groupHistoricalTasks(items: HistoricalUnfinishedTask[]): HistoricalTaskGroup[] {
  const visibleIds = new Set(items.map(item => item.instanceId))
  return items
    .filter(item => !item.parentInstanceId || !visibleIds.has(item.parentInstanceId))
    .map(root => {
      const children = items.filter(item => item.parentInstanceId === root.instanceId)
      const blocked = [root, ...children].find(item => !item.reschedulable)
      return {
        root,
        children,
        reschedulable: !blocked,
        blockedReason: blocked?.blockedReason ?? null,
      }
    })
}

export const historicalStatusLabel = (status: string) => ({
  not_started: '未开始',
  in_progress: '进行中',
  waiting: '等待中',
  blocked: '已阻塞',
}[status] ?? status)
