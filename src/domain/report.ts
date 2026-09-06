export interface DailyMetric { date: string; plannedTasks: number; completedTasks: number; focusMinutes: number; habitEffective: number; habitTotal: number; habitPending: number }
export interface HabitDetail { id: string; title: string; days: string[]; completed: number; missed: number; prerequisiteMissed: number; pending: number; currentStreak: number | null; longestStreak: number; weekLongestStreak: number; breaks: number; previousCompleted: number; previousReviewed: number; streakThrough: string | null }
export interface ReportTask { id: string; title: string; date: string; status: string; importance: string; urgency: string; carried: boolean }
export interface WeeklyReportData {
  weekStart: string; weekEnd: string; daily: DailyMetric[]; plannedTasks: number; completedTasks: number; focusMinutes: number; completionRate: number
  habitEffective: number; habitTotal: number; habitRate: number; focusChangePercent: number | null; completedChangePercent: number | null
  baselineWeeks: number; comparisonDays: number; habitPending: number; scenario: string; headline: string; observation: string
  quote: { id: string; text: string; author: string; source: string }
  goals: Array<{ title: string; progressPercent: number; trophy: 'bronze' | 'silver' | 'gold' | null }>
  habits: HabitDetail[]; tasks: ReportTask[]
  focusDetail: { sessions: number; completedSessions: number; abandonedSessions: number; averageMinutes: number; longestMinutes: number; pauses: number; switches: number }
  history: Array<{ weekStart: string; planned: number; completed: number; focusMinutes: number; habitCompleted: number; habitReviewed: number }>
}
export const weekdays = ['一', '二', '三', '四', '五', '六', '日']
export const habitLabels: Record<string, string> = { done: '已完成', missed: '未完成', prerequisite: '前置项未完成', pending: '待回顾', upcoming: '未到回顾时间', inactive: '未启用' }
export const habitSymbols: Record<string, string> = { done: '✓', missed: '×', prerequisite: '!', pending: '?', upcoming: '·', inactive: '—' }
export const taskLabels: Record<string, string> = { not_started: '未开始', in_progress: '进行中', waiting: '等待中', blocked: '遇到阻碍', completed: '已完成', deferred: '已延期', cancelled: '已取消' }
export const trophies = { bronze: '铜杯', silver: '银杯', gold: '金杯' }
export const minutesText = (value: number) => value >= 60 ? `${Math.floor(value / 60)}小时${value % 60 ? `${value % 60}分` : ''}` : `${value}分钟`
export const trend = (value: number | null) => value === null ? '上周无记录' : value === 0 ? '与上周持平' : `比上周${value > 0 ? '增加' : '减少'} ${Math.abs(value)}%`
export const rateText = (done: number, total: number) => total > 0 ? `${Math.floor(done * 100 / total)}%` : '—'
export const streakText = (habit: Pick<HabitDetail, 'currentStreak' | 'streakThrough'>) => habit.streakThrough === null ? '尚未开始' : habit.currentStreak === null ? '待回顾' : `${habit.currentStreak} 天`
export const priorityGroups = [
  { title: '重要 · 紧急', importance: 'important', urgency: 'urgent' },
  { title: '重要 · 稍缓', importance: 'important', urgency: 'relaxed' },
  { title: '次要 · 紧急', importance: 'secondary', urgency: 'urgent' },
  { title: '次要 · 稍缓', importance: 'secondary', urgency: 'relaxed' },
]
