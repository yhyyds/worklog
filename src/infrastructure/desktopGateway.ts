import { invoke } from '@tauri-apps/api/core'
import type { LongTermItem, MoveToLongTerm, SaveLongTermItem } from '../domain/longTerm'
import type { DayState, TaskStatus } from '../domain/model'
import type { CloseDayRequest, CloseDayResult, CreateTaskRequest, EndOfDayPreview, SaveScheduleRequest, TimerSettings, UpdateTaskRequest, WorkEntryRequest, WorklogGateway } from '../application/gateway'

export class DesktopGateway implements WorklogGateway {
  listLongTermWork() { return invoke<LongTermItem[]>('list_long_term_work') }
  saveLongTermWork(input: SaveLongTermItem) { return invoke<LongTermItem[]>('save_long_term_work', { input }) }
  moveTaskToLongTerm(input: MoveToLongTerm) { return invoke<DayState>('move_task_to_long_term', { input }) }
  getDaySnapshot(workDate: string) { return invoke<DayState>('get_day_snapshot', { workDate }) }
  createTask(input: CreateTaskRequest) { return invoke<DayState>('create_task', { input }) }
  updateTask(input: UpdateTaskRequest) { return invoke<DayState>('update_task', { input }) }
  setTaskStatus(workDate: string, instanceId: string, status: TaskStatus) { return invoke<DayState>('set_task_status', { input: { workDate, instanceId, status } }) }
  saveSchedule(input: SaveScheduleRequest) { return invoke<DayState>('save_daily_schedule', { input }) }
  cancelSchedule(workDate: string, scheduleId: string) { return invoke<DayState>('cancel_daily_schedule', { input: { workDate, scheduleId } }) }
  addWorkEntry(input: WorkEntryRequest) { return invoke<DayState>('add_work_entry', { input }) }
  startFocus(workDate: string, taskId: string, plannedSeconds: number) { return invoke<DayState>('start_focus', { input: { workDate, taskId, plannedSeconds } }) }
  pauseFocus(workDate: string, reason: string) { return invoke<DayState>('pause_focus', { input: { workDate, reason } }) }
  resumeFocus(workDate: string) { return invoke<DayState>('resume_focus', { input: { workDate } }) }
  switchFocus(workDate: string, taskId: string) { return invoke<DayState>('switch_focus', { input: { workDate, taskId } }) }
  completeFocus(workDate: string, reason: 'elapsed' | 'early_complete' | 'abandoned') { return invoke<DayState>('complete_focus', { input: { workDate, reason } }) }
  pauseRest(workDate: string) { return invoke<DayState>('pause_rest', { input: { workDate } }) }
  resumeRest(workDate: string) { return invoke<DayState>('resume_rest', { input: { workDate } }) }
  completeRest(workDate: string) { return invoke<DayState>('complete_rest', { input: { workDate } }) }
  skipRest(workDate: string) { return invoke<DayState>('skip_rest', { input: { workDate } }) }
  getTimerSettings() { return invoke<TimerSettings>('get_timer_settings') }
  saveTimerSettings(settings: TimerSettings) { return invoke<TimerSettings>('save_timer_settings', { settings }) }
  previewEndOfDay(workDate: string) { return invoke<EndOfDayPreview>('preview_end_of_day', { workDate }) }
  closeDay(input: CloseDayRequest) { return invoke<CloseDayResult>('close_day', { input }) }
}
