import { useEffect, useMemo, useState, type FormEvent } from 'react'
import {
  emptyDay, id, localDate, nextDisplayCode, remainingSeconds, timelineEvent,
  type DayState, type DayTask, type EntryType, type Importance, type ReviewLevel, type Urgency,
} from './domain/model'
import { loadDay, saveDay } from './infrastructure/dayStorage'

const quadrants: Array<{ key: string; title: string; note: string; importance: Importance; urgency: Urgency }> = [
  { key: 'important-urgent', title: '重要 · 紧急', note: '今天优先推进', importance: 'important', urgency: 'urgent' },
  { key: 'important-relaxed', title: '重要 · 稍缓', note: '安排深度工作', importance: 'important', urgency: 'relaxed' },
  { key: 'secondary-urgent', title: '次要 · 紧急', note: '快速处理或委派', importance: 'secondary', urgency: 'urgent' },
  { key: 'secondary-relaxed', title: '次要 · 稍缓', note: '保持边界，适时整理', importance: 'secondary', urgency: 'relaxed' },
]

const nav = ['我的一天', '专注', '工作想法', '随笔', 'Obsidian', '设置']
const entryLabels: Record<EntryType, string> = { progress: '进度', idea: '想法', decision: '决定', blocker: '阻塞', result: '结果' }
const levelLabels: Record<ReviewLevel, string> = { key: '关键', normal: '普通', scratch: '草稿' }
const formatClock = (iso: string) => new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false }).format(new Date(iso))
const formatSeconds = (seconds: number) => `${String(Math.floor(seconds / 60)).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`

async function notify(title: string, body: string) {
  if ('__TAURI_INTERNALS__' in window) {
    const { isPermissionGranted, requestPermission, sendNotification } = await import('@tauri-apps/plugin-notification')
    let allowed = await isPermissionGranted()
    if (!allowed) allowed = (await requestPermission()) === 'granted'
    if (allowed) sendNotification({ title, body })
    return
  }
  if ('Notification' in window && Notification.permission === 'granted') new Notification(title, { body })
}

function App() {
  const workDate = localDate()
  const [day, setDay] = useState<DayState>(() => typeof localStorage === 'undefined' ? emptyDay(workDate) : loadDay(workDate))
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null)
  const [childFor, setChildFor] = useState<string | null>(null)
  const [tick, setTick] = useState(Date.now())
  const [thought, setThought] = useState('')
  const [entryType, setEntryType] = useState<EntryType>('idea')
  const [reviewLevel, setReviewLevel] = useState<ReviewLevel>('normal')

  useEffect(() => saveDay(day), [day])
  useEffect(() => {
    const timer = window.setInterval(() => setTick(Date.now()), 1000)
    return () => window.clearInterval(timer)
  }, [])

  const activeTask = useMemo(() => day.tasks.find((task) => task.id === day.focus?.taskId) ?? null, [day])
  const selectedTask = day.tasks.find((task) => task.id === selectedTaskId) ?? day.tasks.find((task) => task.status !== 'completed') ?? null
  const left = day.focus ? remainingSeconds(day.focus, tick) : 1500

  useEffect(() => {
    if (!day.focus || day.focus.status !== 'running' || left > 0) return
    const task = day.tasks.find((item) => item.id === day.focus?.taskId)
    setDay((current) => ({ ...current, focus: null, timeline: [...current.timeline, timelineEvent('focus.completed', '完成一轮工作，共25分钟')] }))
    void notify('专注结束', task ? `${task.displayCode} ${task.title}` : '该休息一下了')
  }, [day.focus, day.tasks, left])

  function createTask(title: string, importance: Importance, urgency: Urgency, parentId: string | null, plannedStart: string | null, plannedEnd: string | null) {
    setDay((current) => {
      const parent = parentId ? current.tasks.find((task) => task.id === parentId) : null
      const displayCode = nextDisplayCode(current.tasks, parentId)
      const task: DayTask = {
        id: id(), permanentTaskId: id(), parentId, displayCode, title, status: 'not_started',
        importance: parent?.importance ?? importance, urgency: parent?.urgency ?? urgency,
        plannedStart, plannedEnd, createdAt: new Date().toISOString(),
      }
      return { ...current, tasks: [...current.tasks, task], timeline: [...current.timeline, timelineEvent('task.created', `新增任务${displayCode}：${title}`)] }
    })
    setChildFor(null)
  }

  function toggleTask(task: DayTask) {
    const completed = task.status !== 'completed'
    setDay((current) => ({
      ...current,
      tasks: current.tasks.map((item) => item.id === task.id ? { ...item, status: completed ? 'completed' : 'not_started' } : item),
      timeline: [...current.timeline, timelineEvent(completed ? 'task.completed' : 'task.reopened', `${completed ? '完成' : '恢复'}${task.displayCode}：${task.title}`)],
    }))
  }

  function startFocus(task: DayTask) {
    const now = Date.now()
    setSelectedTaskId(task.id)
    setDay((current) => ({
      ...current,
      tasks: current.tasks.map((item) => item.id === task.id && item.status === 'not_started' ? { ...item, status: 'in_progress' } : item),
      focus: { id: id(), taskId: task.id, status: 'running', plannedSeconds: 1500, remainingSeconds: 1500, targetEndAt: new Date(now + 1500_000).toISOString(), startedAt: new Date(now).toISOString() },
      timeline: [...current.timeline, timelineEvent('focus.started', `开始一轮工作，任务内容：${task.displayCode} ${task.title}`)],
    }))
  }

  function pauseOrResume() {
    setDay((current) => {
      if (!current.focus) return current
      if (current.focus.status === 'running') {
        const remaining = remainingSeconds(current.focus)
        return { ...current, focus: { ...current.focus, status: 'paused', remainingSeconds: remaining, targetEndAt: null }, timeline: [...current.timeline, timelineEvent('focus.paused', '暂停本轮工作', 'detail')] }
      }
      return { ...current, focus: { ...current.focus, status: 'running', targetEndAt: new Date(Date.now() + current.focus.remainingSeconds * 1000).toISOString() }, timeline: [...current.timeline, timelineEvent('focus.resumed', '继续本轮工作', 'detail')] }
    })
  }

  function endFocus(abandoned = false) {
    if (!day.focus) return
    const actual = day.focus.plannedSeconds - left
    setDay((current) => ({ ...current, focus: null, timeline: [...current.timeline, timelineEvent(abandoned ? 'focus.abandoned' : 'focus.completed', abandoned ? `放弃本轮工作，已进行${Math.max(1, Math.round(actual / 60))}分钟` : `提前完成本轮工作，共${Math.max(1, Math.round(actual / 60))}分钟`)] }))
  }

  function addThought(event: FormEvent) {
    event.preventDefault()
    const content = thought.trim()
    if (!content) return
    const task = selectedTask
    const visibility = reviewLevel === 'scratch' ? 'hidden' : reviewLevel === 'key' ? 'summary' : 'detail'
    setDay((current) => ({ ...current, timeline: [...current.timeline, timelineEvent('work_entry.created', `${task ? `${task.displayCode} · ` : ''}${content}`, visibility, `${entryLabels[entryType]} · ${levelLabels[reviewLevel]}`)] }))
    setThought('')
  }

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><span className="brand-mark">W</span><div><strong>Worklog</strong><small>把一天自然记录下来</small></div></div>
      <nav>{nav.map((item, index) => <button key={item} className={index === 0 ? 'active' : ''}><span>{['☀', '◉', '◇', '▤', '⬡', '⚙'][index]}</span>{item}</button>)}</nav>
      <div className="sidebar-foot"><span className="status-dot"/>数据仅保存在本地</div>
    </aside>

    <main>
      <header className="topbar"><div><p>{new Intl.DateTimeFormat('zh-CN', { month: 'long', day: 'numeric', weekday: 'long' }).format(new Date())}</p><h1>我的一天</h1></div><div className="top-actions"><span>{day.tasks.filter((task) => task.status === 'completed').length}/{day.tasks.length} 已完成</span><button className="round">•••</button></div></header>

      <section className="focus-strip">
        <div className="timer-ring"><strong>{formatSeconds(left)}</strong><span>{day.focus ? (day.focus.status === 'running' ? '专注中' : '已暂停') : '准备专注'}</span></div>
        <div className="focus-copy"><small>当前专注</small><h2>{activeTask ? `${activeTask.displayCode} ${activeTask.title}` : selectedTask ? `${selectedTask.displayCode} ${selectedTask.title}` : '先选择一项今日任务'}</h2><p>{day.focus ? '记录会自动关联到当前任务' : '选择任务后，一次点击即可开始 25 分钟'}</p></div>
        <div className="focus-actions">
          {!day.focus && selectedTask && <button className="primary" onClick={() => startFocus(selectedTask)}>▶ 开始专注</button>}
          {day.focus && <><button className="secondary" onClick={pauseOrResume}>{day.focus.status === 'running' ? 'Ⅱ 暂停' : '▶ 继续'}</button><button className="text-button" onClick={() => endFocus(false)}>提前完成</button><button className="text-button danger" onClick={() => endFocus(true)}>放弃</button></>}
        </div>
      </section>

      <div className="content-grid">
        <section className="tasks-panel">
          <div className="section-heading"><div><h2>今日安排</h2><p>按重要性与紧急性组织，不让任务列表变得太吵</p></div><TaskForm compact onCreate={createTask}/></div>
          <div className="quadrants">{quadrants.map((quadrant) => {
            const tasks = day.tasks.filter((task) => !task.parentId && task.importance === quadrant.importance && task.urgency === quadrant.urgency)
            return <article className="quadrant" key={quadrant.key}>
              <header><div><h3>{quadrant.title}</h3><small>{quadrant.note}</small></div><span>{tasks.length}</span></header>
              <div className="task-list">{tasks.length === 0 && <p className="empty">暂时没有事项</p>}{tasks.map((task) => <div key={task.id} className={`task-card ${task.status === 'completed' ? 'done' : ''} ${selectedTaskId === task.id ? 'selected' : ''}`} onClick={() => setSelectedTaskId(task.id)}>
                <button className="check" onClick={(event) => { event.stopPropagation(); toggleTask(task) }}>{task.status === 'completed' ? '✓' : ''}</button>
                <div className="task-body"><strong><span>{task.displayCode}</span> {task.title}</strong>{task.plannedStart && <small>{task.plannedStart}–{task.plannedEnd}</small>}
                  {day.tasks.filter((child) => child.parentId === task.id).map((child) => <div className="child" key={child.id}><button className="mini-check" onClick={(event) => { event.stopPropagation(); toggleTask(child) }}>{child.status === 'completed' ? '✓' : ''}</button><span className={child.status === 'completed' ? 'strike' : ''}>{child.displayCode} {child.title}</span></div>)}
                  {childFor === task.id ? <TaskForm parentId={task.id} importance={task.importance} urgency={task.urgency} onCreate={createTask}/> : <button className="add-child" onClick={(event) => { event.stopPropagation(); setChildFor(task.id) }}>＋ 新建子任务</button>}
                </div>
              </div>)}</div>
            </article>
          })}</div>
        </section>

        <aside className="timeline-panel">
          <div className="section-heading"><div><h2>今日记录</h2><p>只展示值得回顾的内容</p></div><span className="live"><i/>实时</span></div>
          <form className="thought-form" onSubmit={addThought}><textarea value={thought} onChange={(event) => setThought(event.target.value)} placeholder="记录一个工作想法…"/><div><select value={entryType} onChange={(event) => setEntryType(event.target.value as EntryType)}>{Object.entries(entryLabels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select><select value={reviewLevel} onChange={(event) => setReviewLevel(event.target.value as ReviewLevel)}>{Object.entries(levelLabels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select><button type="submit">记录</button></div></form>
          <div className="timeline">{day.timeline.filter((item) => item.visibility !== 'hidden').length === 0 && <div className="timeline-empty"><b>今天会自然形成一份日记</b><span>完成任务、开始专注或写下想法后，记录会出现在这里。</span></div>}{[...day.timeline].reverse().filter((item) => item.visibility !== 'hidden').map((item) => <div className="event" key={item.id}><time>{formatClock(item.occurredAt)}</time><i/><div><p>{item.title}</p>{item.detail && <small>{item.detail}</small>}</div></div>)}</div>
        </aside>
      </div>
    </main>
  </div>
}

interface TaskFormProps {
  compact?: boolean
  parentId?: string | null
  importance?: Importance
  urgency?: Urgency
  onCreate: (title: string, importance: Importance, urgency: Urgency, parentId: string | null, start: string | null, end: string | null) => void
}

function TaskForm({ compact, parentId = null, importance: initialImportance = 'important', urgency: initialUrgency = 'urgent', onCreate }: TaskFormProps) {
  const [open, setOpen] = useState(!compact)
  const [title, setTitle] = useState('')
  const [importance, setImportance] = useState<Importance>(initialImportance)
  const [urgency, setUrgency] = useState<Urgency>(initialUrgency)
  const [timed, setTimed] = useState(false)
  const [start, setStart] = useState('09:00')
  const [end, setEnd] = useState('10:00')
  if (!open) return <button className="primary small" onClick={() => setOpen(true)}>＋ 新建日程</button>
  return <form className={`task-form ${parentId ? 'child-form' : ''}`} onSubmit={(event) => { event.preventDefault(); if (!title.trim()) return; onCreate(title.trim(), importance, urgency, parentId, timed ? start : null, timed ? end : null); setTitle(''); if (compact) setOpen(false) }} onClick={(event) => event.stopPropagation()}>
    <input autoFocus={!compact} value={title} onChange={(event) => setTitle(event.target.value)} placeholder={parentId ? '子任务内容' : '输入新事项…'}/>
    {!parentId && <><select value={importance} onChange={(event) => setImportance(event.target.value as Importance)}><option value="important">重要</option><option value="secondary">次要</option></select><select value={urgency} onChange={(event) => setUrgency(event.target.value as Urgency)}><option value="urgent">紧急</option><option value="relaxed">稍缓</option></select><label className="time-toggle"><input type="checkbox" checked={timed} onChange={(event) => setTimed(event.target.checked)}/>安排时间</label>{timed && <span className="time-range"><input type="time" value={start} onChange={(event) => setStart(event.target.value)}/><b>–</b><input type="time" value={end} onChange={(event) => setEnd(event.target.value)}/></span>}</>}
    <button type="submit">保存</button>{compact && <button type="button" className="cancel" onClick={() => setOpen(false)}>取消</button>}
  </form>
}

export default App
