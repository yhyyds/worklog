import { useEffect, useMemo, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { habitLabels, habitSymbols, minutesText, priorityGroups, rateText, streakText, taskLabels, trend, trophies, weekdays, type ReportTask, type WeeklyReportData } from '../domain/report'
import { renderReportPng } from '../infrastructure/reportImage'
import ShareOverview, { CategoryCards } from './ShareOverview'
import type { Overview } from '../domain/sharing'
import { renderOverviewPng } from '../infrastructure/shareImage'

const desktop = '__TAURI_INTERNALS__' in window
const dateText = (date: Date) => [date.getFullYear(), String(date.getMonth() + 1).padStart(2, '0'), String(date.getDate()).padStart(2, '0')].join('-')
const monday = (date = new Date()) => { const next = new Date(date); next.setDate(next.getDate() - (next.getDay() + 6) % 7); return dateText(next) }
const moveWeek = (value: string, offset: number) => { const [year, month, day] = value.split('-').map(Number); return dateText(new Date(year, month - 1, day + offset * 7, 12)) }

export default function WeeklyReport() {
  const [open, setOpen] = useState(false)
  const [weekStart, setWeekStart] = useState(monday)
  const [report, setReport] = useState<WeeklyReportData | null>(null)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const [error, setError] = useState('')
  const requestId = useRef(0)
  const sharingRef = useRef(true)
  const [sharing, setSharing] = useState(true)
  const [overview, setOverview] = useState<Overview | null>(null)
  const [preview, setPreview] = useState<{ blob: Blob; url: string; week: string; sharing: boolean } | null>(null)
  async function load(value: string, mode = sharingRef.current) {
    if (!desktop) return
    const id = ++requestId.current
    setBusy(true); setError(''); setMessage(''); setReport(null); setOverview(null); setPreview(null)
    try {
      const [summary, result] = await Promise.all([
        invoke<Overview>('get_report_overview', { weekStart: value, sharing: mode }),
        mode ? Promise.resolve(null) : invoke<WeeklyReportData>('get_weekly_report', { weekStart: value }),
      ])
      if (id === requestId.current) { setReport(result); setOverview(summary) }
    }
    catch (reason) { if (id === requestId.current) setError(String(reason)) }
    finally { if (id === requestId.current) setBusy(false) }
  }
  useEffect(() => {
    const show = () => { const value = monday(); setWeekStart(value); setOpen(true); setBusy(desktop); setReport(null); setOverview(null); setPreview(null); if (desktop) void invoke<boolean>('get_share_preference').catch(() => true).then(mode => { sharingRef.current = mode; setSharing(mode); void load(value, mode) }) }
    window.addEventListener('worklog:open-weekly-report', show)
    return () => window.removeEventListener('worklog:open-weekly-report', show)
  }, [])
  const maxFocus = useMemo(() => Math.max(1, ...(report?.daily.map(day => day.focusMinutes) ?? [1])), [report])
  function chooseWeek(value: string) { if (!/^\d{4}-\d{2}-\d{2}$/.test(value) || value > monday()) return; setWeekStart(value); void load(value) }
  async function changeSharing(mode: boolean) {
    sharingRef.current = mode; setSharing(mode); setPreview(null)
    await load(weekStart, mode)
    try { await invoke('save_share_preference', { sharing: mode }) } catch (e) { setError(`分享偏好未保存：${String(e)}`) }
  }
  async function exportImage() {
    if (!overview || overview.sharing !== sharing || (!sharing && !report)) return
    setBusy(true); setError(''); setMessage('')
    try {
      const blob = sharing ? await renderOverviewPng(overview) : await renderReportPng(report!, overview.categories)
      // data: images are explicitly allowed by the desktop CSP; blob: is not.
      const url = await new Promise<string>((resolve, reject) => { const reader = new FileReader(); reader.onload = () => resolve(String(reader.result)); reader.onerror = () => reject(new Error('图片预览读取失败')); reader.readAsDataURL(blob) })
      setPreview({ blob, url, week: overview.weekStart, sharing })
    } catch (reason) { setError(String(reason)) }
    finally { setBusy(false) }
  }
  async function savePreview() {
    if (!preview) return
    setBusy(true); setError('')
    try {
      const { save } = await import('@tauri-apps/plugin-dialog')
      const path = await save({ defaultPath: `Worklog-一周小笺-${preview.week}.png`, filters: [{ name: 'PNG 图片', extensions: ['png'] }] })
      if (!path) return
      await invoke('save_weekly_report_image', { path, bytes: Array.from(new Uint8Array(await preview.blob.arrayBuffer())) })
      setMessage(`图片已保存：${path}`)
      setPreview(null)
    } catch (reason) { setError(String(reason)) }
    finally { setBusy(false) }
  }
  if (!open) return null
  const focus = report?.focusDetail
  const bestDay = report?.daily.filter(day => day.focusMinutes > 0).sort((a, b) => b.focusMinutes - a.focusMinutes)[0]
  return <div className="weekly-backdrop" onMouseDown={() => { if (!busy) setOpen(false) }}><section className="weekly-workspace" role="dialog" aria-modal="true" aria-label="一周小笺" onMouseDown={event => event.stopPropagation()}>
    <header><div><small>WORKLOG</small><h2>一周小笺</h2></div><div><button disabled={busy} aria-label="上一周" onClick={() => chooseWeek(moveWeek(weekStart, -1))}>←</button><input aria-label="周报日期" type="date" disabled={busy} max={dateText(new Date())} value={weekStart} onChange={event => { if (event.target.value) chooseWeek(monday(new Date(`${event.target.value}T12:00:00`))) }}/><button disabled={busy || weekStart >= monday()} aria-label="下一周" onClick={() => chooseWeek(moveWeek(weekStart, 1))}>→</button><button disabled={busy || !overview} onClick={() => void exportImage()}>预览并导出</button><button className="weekly-close" aria-label="关闭周报" disabled={busy} onClick={() => setOpen(false)}>×</button></div></header>
    <div className="share-controls"><label><input type="checkbox" checked={sharing} disabled={busy} onChange={e => void changeSharing(e.target.checked)}/>隐私分享</label><span>{sharing ? '名称按分类分享设置展示' : '自己查看 · 包含具体名称'}</span><button disabled={busy} onClick={() => { setOpen(false); window.dispatchEvent(new Event('worklog:open-categories')) }}>管理分类与分享</button></div>
    {error && <p className="weekly-error" role="alert">{error}</p>}{message && <p className="weekly-success">{message}</p>}
    {!desktop && <p className="weekly-empty">请在桌面版查看周报。</p>}
    {busy && !overview && <p className="weekly-empty">正在读取…</p>}
    {sharing && overview && <ShareOverview report={overview}/>}
    {!sharing && report && focus && <div className="weekly-page">
      <section className={`weekly-hero ${report.scenario}`}><small>{report.weekStart} — {report.weekEnd}</small><h3>{report.headline}</h3>{report.observation && <p>{report.observation}</p>}</section>
      <section className="weekly-numbers">
        <div><small>完成事项</small><strong>{report.completedTasks}<i>/{report.plannedTasks}</i></strong><span>{trend(report.completedChangePercent)}</span></div>
        <div><small>专注时间</small><strong>{minutesText(report.focusMinutes)}</strong><span>{trend(report.focusChangePercent)}</span></div>
        <div><small>计划完成率</small><strong>{rateText(report.completedTasks, report.plannedTasks)}</strong><span>{report.plannedTasks - report.completedTasks} 项未完成</span></div>
        <div><small>习惯打卡</small><strong>{rateText(report.habitEffective, report.habitTotal)}</strong><span>完成 {report.habitEffective} 次 · 未完成 {report.habitTotal - report.habitEffective} 次</span></div>
      </section>
      {overview && <CategoryCards categories={overview.categories}/>}
      <section className="weekly-section"><header><h3>习惯记录</h3><span>{report.habits.length} 个习惯 · {report.habitPending} 次待回顾</span></header>
        <div className="habit-week-summary"><span><b>{report.habits.filter(h => h.breaks > 0).length}</b> 个习惯中断</span><span><b>{report.habits.reduce((n, h) => n + h.breaks, 0)}</b> 次中断</span><span><b>{report.habits.reduce((n, h) => n + h.missed + h.prerequisiteMissed, 0)}</b> 次未完成</span></div>
        <div className="habit-legend">{['done', 'missed', 'prerequisite', 'pending'].map(status => <span key={status}><i className={`habit-day ${status}`}>{habitSymbols[status]}</i>{habitLabels[status]}</span>)}</div>
        {!report.habits.length && <p className="weekly-muted">本周没有打卡项。</p>}
        {report.habits.map(habit => <article className="habit-week-card" key={habit.id}>
          <div className="habit-week-title"><h4>{habit.title}</h4><span>{habit.completed} 次完成 / {habit.completed + habit.missed + habit.prerequisiteMissed} 次已回顾</span></div>
          <div className="habit-week-days">{habit.days.map((status, index) => <div key={index}><small>周{weekdays[index]}</small><span className={`habit-day ${status}`} title={`${report.daily[index].date}：${habitLabels[status]}`} aria-label={`${habit.title}，${report.daily[index].date}，${habitLabels[status]}`}>{habitSymbols[status]}</span></div>)}</div>
          <dl className="habit-week-stats"><div><dt>连续完成</dt><dd title={habit.streakThrough ? `截至 ${habit.streakThrough}` : ''}>{streakText(habit)}</dd></div><div><dt>本周最长</dt><dd>{habit.weekLongestStreak} 天</dd></div><div><dt>历史最长</dt><dd>{habit.longestStreak} 天</dd></div><div><dt>本周中断</dt><dd>{habit.breaks} 次</dd></div><div><dt>上周同期</dt><dd>{habit.previousReviewed ? `${habit.previousCompleted}/${habit.previousReviewed} 次` : '—'}</dd></div></dl>
          {habit.prerequisiteMissed > 0 && <p className="habit-note">有 {habit.prerequisiteMissed} 天做到了这一项，但前置打卡未完成。</p>}
        </article>)}
      </section>
      <section className="weekly-section"><header><h3>专注与每日安排</h3><span>{report.daily.filter(day => day.focusMinutes > 0).length} 天有专注记录</span></header>
        <dl className="focus-stat-grid"><div><dt>完成专注</dt><dd>{focus.completedSessions} 轮</dd></div><div><dt>中途结束</dt><dd>{focus.abandonedSessions} 轮</dd></div><div><dt>平均每轮</dt><dd>{focus.averageMinutes} 分钟</dd></div><div><dt>最长一轮</dt><dd>{focus.longestMinutes} 分钟</dd></div><div><dt>暂停</dt><dd>{focus.pauses} 次</dd></div><div><dt>切换任务</dt><dd>{focus.switches} 次</dd></div></dl>
        <div className="weekly-chart"><div>{report.daily.map((day, index) => <div key={day.date}><span>周{weekdays[index]}</span><i><b style={{ height: `${day.focusMinutes / maxFocus * 100}%` }}/></i><strong>{index < report.comparisonDays ? `${day.focusMinutes}分` : '—'}</strong></div>)}</div></div>
        {bestDay && <p className="weekly-muted">专注最多：{bestDay.date.slice(5)}，{minutesText(bestDay.focusMinutes)}。</p>}
        <div className="weekly-table-scroll"><table className="weekly-table"><thead><tr><th>日期</th><th>完成 / 计划</th><th>完成率</th><th>专注</th><th>打卡完成</th></tr></thead><tbody>{report.daily.map((day, index) => <tr key={day.date}><th>{day.date.slice(5)} 周{weekdays[index]}</th><td>{index < report.comparisonDays ? `${day.completedTasks} / ${day.plannedTasks}` : '—'}</td><td>{rateText(day.completedTasks, day.plannedTasks)}</td><td>{index < report.comparisonDays ? minutesText(day.focusMinutes) : '—'}</td><td>{day.habitTotal ? `${day.habitEffective} / ${day.habitTotal}` : day.habitPending ? '待回顾' : '—'}</td></tr>)}</tbody></table></div>
      </section>
      <section className="weekly-section"><header><h3>任务分布</h3><span>完成 / 计划</span></header><div className="weekly-priorities">{priorityGroups.map(group => { const tasks = report.tasks.filter(task => task.importance === group.importance && task.urgency === group.urgency); const done = tasks.filter(task => task.status === 'completed').length; return <div key={group.title}><span>{group.title}</span><strong>{done}<small> / {tasks.length}</small></strong><progress max={Math.max(1, tasks.length)} value={done}/></div> })}</div>
        <TaskList title="已完成的事项" tasks={report.tasks.filter(task => task.status === 'completed')}/><TaskList title="未完成的事项" tasks={report.tasks.filter(task => task.status !== 'completed')} expanded/>
      </section>
      <section className="weekly-section"><header><h3>近四周对比</h3><span>{report.comparisonDays < 7 ? `每周前 ${report.comparisonDays} 天` : '完整周'}</span></header><div className="weekly-table-scroll"><table className="weekly-table"><thead><tr><th>周起始日</th><th>完成事项</th><th>完成率</th><th>专注</th><th>打卡完成</th></tr></thead><tbody>{[...report.history, { weekStart: report.weekStart, planned: report.plannedTasks, completed: report.completedTasks, focusMinutes: report.focusMinutes, habitCompleted: report.habitEffective, habitReviewed: report.habitTotal }].map(week => <tr key={week.weekStart} className={week.weekStart === report.weekStart ? 'current-week' : ''}><th>{week.weekStart.slice(5)}{week.weekStart === report.weekStart ? ' 本周' : ''}</th><td>{week.completed} / {week.planned}</td><td>{rateText(week.completed, week.planned)}</td><td>{minutesText(week.focusMinutes)}</td><td>{week.habitReviewed ? `${week.habitCompleted} / ${week.habitReviewed}` : '—'}</td></tr>)}</tbody></table></div></section>
      {report.goals.length > 0 && <section className="weekly-section"><header><h3>长期目标</h3><span>当前进度</span></header>{report.goals.map((goal, index) => <div className="weekly-goal-row" key={index}><span>{goal.title}</span><progress max={100} value={Math.min(goal.progressPercent, 100)}/><strong>{goal.progressPercent}% {goal.trophy ? trophies[goal.trophy] : ''}</strong></div>)}</section>}
      <blockquote><p>“{report.quote.text}”</p><footer>—— {report.quote.author} · {report.quote.source}</footer></blockquote>
      <details className="weekly-definitions"><summary>统计说明</summary><p>本周截至 {report.daily[report.comparisonDays - 1].date}，与上周相同天数比较。完成率按每日安排统计；有子任务时只统计子任务，已取消和收回待办箱的未完成事项不计入。</p><p>连续天数可跨周，统计到本周末或昨天；遇到尚未回顾的日期显示“待回顾”。中断是连续完成后，紧接着一天确认未完成；未回顾不算中断。前置项未完成时，后置打卡也算未完成。</p><p>专注时长来自已记录的计时，不等同于全部工作时间；每轮平均时长包含中途结束的专注。长期目标显示当前进度，历史周不显示。</p></details>
    </div>}
    {preview && <div className="report-image-preview" role="dialog" aria-modal="true" aria-label="图片导出预览"><header><span>{preview.sharing ? '分享图片预览' : '个人图片预览 · 包含具体名称'}</span><button disabled={busy} onClick={() => void savePreview()}>保存图片</button><button disabled={busy} onClick={() => setPreview(null)}>返回</button></header><div><img src={preview.url} alt="即将导出的完整周报"/></div></div>}
  </section></div>
}
function TaskList({ title, tasks, expanded = false }: { title: string; tasks: ReportTask[]; expanded?: boolean }) {
  return <details className="weekly-task-list" open={expanded}><summary>{title}<span>{tasks.length} 项</span></summary>{tasks.length ? tasks.map(task => <div key={task.id}><time>{task.date.slice(5)}</time><span>{task.title}</span><small>{taskLabels[task.status] ?? task.status}</small></div>) : <p className="weekly-muted">暂无事项。</p>}</details>
}
