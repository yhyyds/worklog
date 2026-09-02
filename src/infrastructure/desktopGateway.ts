import { invoke } from '@tauri-apps/api/core'
import type { DayState, TaskStatus } from '../domain/model'
import type { CloseDayRequest, CloseDayResult, CreateTaskRequest, EndOfDayPreview, WorkEntryRequest, WorklogGateway } from '../application/gateway'

export class DesktopGateway implements WorklogGateway {
  getDaySnapshot(workDate: string) { return invoke<DayState>('get_day_snapshot', { workDate }) }
  createTask(input: CreateTaskRequest) { return invoke<DayState>('create_task', { input }) }
  setTaskStatus(workDate: string, instanceId: string, status: TaskStatus) { return invoke<DayState>('set_task_status', { input: { workDate, instanceId, status } }) }
  addWorkEntry(input: WorkEntryRequest) { return invoke<DayState>('add_work_entry', { input }) }
  startFocus(workDate: string, taskId: string, plannedSeconds: number) { return invoke<DayState>('start_focus', { input: { workDate, taskId, plannedSeconds } }) }
  pauseFocus(workDate: string) { return invoke<DayState>('pause_focus', { input: { workDate } }) }
  resumeFocus(workDate: string) { return invoke<DayState>('resume_focus', { input: { workDate } }) }
  switchFocus(workDate: string, taskId: string) { return invoke<DayState>('switch_focus', { input: { workDate, taskId } }) }
  completeFocus(workDate: string, reason: 'elapsed' | 'early_complete' | 'abandoned') { return invoke<DayState>('complete_focus', { input: { workDate, reason } }) }
  previewEndOfDay(workDate: string) { return invoke<EndOfDayPreview>('preview_end_of_day', { workDate }) }
  closeDay(input: CloseDayRequest) { return invoke<CloseDayResult>('close_day', { input }) }
}
