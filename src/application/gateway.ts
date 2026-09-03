import type { DayState, EntryType, Importance, ReviewLevel, TaskStatus, Urgency } from '../domain/model'

export interface CreateTaskRequest {
  workDate: string
  title: string
  importance: Importance
  urgency: Urgency
  parentId: string | null
  plannedStart: string | null
  plannedEnd: string | null
}

export interface UpdateTaskRequest {
  workDate: string
  instanceId: string
  title: string
  plannedStart: string | null
  plannedEnd: string | null
}

export interface WorkEntryRequest {
  workDate: string
  content: string
  entryType: EntryType
  reviewLevel: ReviewLevel
  taskId: string | null
}

export interface CarryCandidate {
  instanceId: string
  permanentTaskId: string
  parentId: string | null
  displayCode: string
  title: string
  status: TaskStatus
  importance: Importance
  urgency: Urgency
}

export interface EndOfDayPreview {
  workDate: string
  nextWorkDate: string
  totalCount: number
  completedCount: number
  waitingCount: number
  blockedCount: number
  candidates: CarryCandidate[]
  alreadyClosed: boolean
}

export interface CloseDayRequest {
  workDate: string
  nextWorkDate: string
  selectedInstanceIds: string[]
}

export interface CloseDayResult {
  sourceDay: DayState
  nextDay: DayState
  carriedCount: number
  skippedCount: number
}

export interface TimerSettings {
  workMinutes: number
  shortBreakMinutes: number
  longBreakMinutes: number
  longBreakInterval: number
  autoStartBreak: boolean
}

export interface WorklogGateway {
  getDaySnapshot(workDate: string): Promise<DayState>
  createTask(input: CreateTaskRequest): Promise<DayState>
  updateTask(input: UpdateTaskRequest): Promise<DayState>
  setTaskStatus(workDate: string, instanceId: string, status: TaskStatus): Promise<DayState>
  addWorkEntry(input: WorkEntryRequest): Promise<DayState>
  startFocus(workDate: string, taskId: string, plannedSeconds: number): Promise<DayState>
  pauseFocus(workDate: string, reason: string): Promise<DayState>
  resumeFocus(workDate: string): Promise<DayState>
  switchFocus(workDate: string, taskId: string): Promise<DayState>
  completeFocus(workDate: string, reason: 'elapsed' | 'early_complete' | 'abandoned'): Promise<DayState>
  pauseRest(workDate: string): Promise<DayState>
  resumeRest(workDate: string): Promise<DayState>
  completeRest(workDate: string): Promise<DayState>
  skipRest(workDate: string): Promise<DayState>
  getTimerSettings(): Promise<TimerSettings>
  saveTimerSettings(settings: TimerSettings): Promise<TimerSettings>
  previewEndOfDay(workDate: string): Promise<EndOfDayPreview>
  closeDay(input: CloseDayRequest): Promise<CloseDayResult>
}
