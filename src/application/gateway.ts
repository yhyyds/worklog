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

export interface WorklogGateway {
  getDaySnapshot(workDate: string): Promise<DayState>
  createTask(input: CreateTaskRequest): Promise<DayState>
  setTaskStatus(workDate: string, instanceId: string, status: TaskStatus): Promise<DayState>
  addWorkEntry(input: WorkEntryRequest): Promise<DayState>
  startFocus(workDate: string, taskId: string, plannedSeconds: number): Promise<DayState>
  pauseFocus(workDate: string): Promise<DayState>
  resumeFocus(workDate: string): Promise<DayState>
  switchFocus(workDate: string, taskId: string): Promise<DayState>
  completeFocus(workDate: string, reason: 'elapsed' | 'early_complete' | 'abandoned'): Promise<DayState>
  previewEndOfDay(workDate: string): Promise<EndOfDayPreview>
  closeDay(input: CloseDayRequest): Promise<CloseDayResult>
}
