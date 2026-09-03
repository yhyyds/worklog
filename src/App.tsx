import { useEffect, useMemo, useRef, useState, type FormEvent } from 'react'
import { incompleteFirst, remainingSeconds, type DayTask, type EntryType, type Importance, type ReviewLevel, type Urgency } from './domain/model'
import { useWorklog } from './application/useWorklog'
import { NAV_ITEMS, navigationAction, type NavItem } from './application/navigation'

const quadrants: Array<{ key: string; title: string; note: string; importance: Importance; urgency: Urgency }> = [
  { key: 'important-urgent', title: '重要 · 紧急', note: '今天优先推进', importance: 'important', urgency: 'urgent' },
  { key: 'important-relaxed', title: '重要 · 稍缓', note: '安排深度工作', importance: 'important', urgency: 'relaxed' },
  { key: 'secondary-urgent', title: '次要 · 紧急', note: '快速处理或委派', importance: 'secondary', urgency: 'urgent' },
  { key: 'secondary-relaxed', title: '次要 · 稍缓', note: '保持边界，适时整理', importance: 'secondary', urgency: 'relaxed' },
]
const entryLabels: Record<EntryType, string> = { progress: '进度', idea: '想法', decision: '决定', blocker: '阻塞', result: '结果' }
const levelLabels: Record<ReviewLevel, string> = { key: '关键', normal: '普通', scratch: '草稿' }
const formatClock = (iso: string) => new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false }).format(new Date(iso))
const formatSeconds = (seconds: number) => `${String(Math.floor(seconds / 60)).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`
const ignore = (promise: Promise<unknown>) => { void promise.catch(() => undefined) }

export function CompletionMark() {
  return <svg viewBox="0 0 12 12" aria-hidden="true" focusable="false">
    <path d="M2.1 6.2 4.8 8.7 9.9 3" />
  </svg>
}

async function notify(title: string, body: string) {
  if ('__TAURI_INTERNALS__' in window) {
    const { isPermissionGranted, requestPermission, sendNotification } = await import('@tauri-apps/plugin-notification')
    let allowed = await isPermissionGranted()
    if (!allowed) allowed = (await requestPermission()) === 'granted'
    if (allowed) sendNotification({ title, body })
  } else if ('Notification' in window && Notification.permission === 'granted') {
    new Notification(title, { body })
  }
}

function App() {
  const worklog = useWorklog()
  const { day } = worklog
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null)
  const [childFor, setChildFor] = useState<string | null>(null)
  const [quickCreateQuadrant, setQuickCreateQuadrant] = useState<string | null>(null)
  const [editingTaskId, setEditingTaskId] = useState<string | null>(null)
  const [pauseOpen, setPauseOpen] = useState(false)
  const [pauseReason, setPauseReason] = useState('')
  const [tick, setTick] = useState(Date.now())
  const [thought, setThought] = useState('')
  const [entryType, setEntryType] = useState<EntryType>('idea')
  const [reviewLevel, setReviewLevel] = useState<ReviewLevel>('normal')
  const completing = useRef(false)

  useEffect(() => {
    const timer = window.setInterval(() => setTick(Date.now()), 1000)
    return () => window.clearInterval(timer)
  }, [])

  const activeTask = useMemo(() => day.tasks.find((task) => task.id === day.focus?.taskId) ?? null, [day])
  const selectedTask = day.tasks.find((task) => !task.parentId && task.id === selectedTaskId) ?? day.tasks.find((task) => !task.parentId && task.status !== 'completed') ?? null
  const left = day.rest ? remainingSeconds(day.rest, tick) : day.focus ? remainingSeconds(day.focus, tick) : worklog.workMinutes * 60

  useEffect(() => {
    if ('__TAURI_INTERNALS__' in window) { completing.current = false; return }
    const running = day.rest ?? day.focus
    if (!running) { completing.current = false; return }
    if (running.status !== 'running' || left > 0 || completing.current) return
    completing.current = true
    const operation = day.rest ? worklog.completeRest() : worklog.completeFocus('elapsed')
    operation.catch(() => { completing.current = false })
  }, [day.focus, day.rest, left, worklog])

  function createTask(title: string, importance: Importance, urgency: Urgency, parentId: string | null, plannedStart: string | null, plannedEnd: string | null) {
    ignore(worklog.createTask(title, importance, urgency, parentId, plannedStart, plannedEnd))
    setChildFor(null)
    setQuickCreateQuadrant(null)
  }

  function updateTask(task: DayTask, title: string, plannedStart: string | null, plannedEnd: string | null) {
    ignore(worklog.updateTask(task.id, title, plannedStart, plannedEnd).then(() => setEditingTaskId(null)))
  }

  function toggleTask(task: DayTask) {
    ignore(worklog.setTaskStatus(task.id, task.status === 'completed' ? 'not_started' : 'completed'))
  }

  function pauseFocus(event: FormEvent) {
    event.preventDefault()
    const reason = pauseReason.trim()
    if (!reason) return
    ignore(worklog.pauseFocus(reason).then(() => {
      setPauseReason('')
      setPauseOpen(false)
    }))
  }

  function chooseFocusTask(taskId: string) {
    if (day.focus) ignore(worklog.switchFocus(taskId))
    else setSelectedTaskId(taskId)
  }

  function addThought(event: FormEvent) {
    event.preventDefault()
    const content = thought.trim()
    if (!content) return
    ignore(worklog.addWorkEntry(content, entryType, reviewLevel, selectedTask?.id ?? null))
    setThought('')
  }

  function navigate(item: NavItem) {
    const action = navigationAction(item)
    window.requestAnimationFrame(() => {
      if (action === 'top') window.scrollTo({ top: 0, behavior: 'smooth' })
      else window.dispatchEvent(new Event(action))
    })
  }

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><span className="brand-mark">W</span><div><strong>Worklog</strong><small>把一天自然记录下来</small></div></div>
      <nav>{NAV_ITEMS.map((item, index) => <button type="button" key={item} className={item === '我的一天' ? 'active' : ''} aria-current={item === '我的一天' ? 'page' : undefined} onClick={() => navigate(item)}><span>{['☀', '▤', '⬡', '⚙'][index]}</span>{item}</button>)}</nav>
      <div className="sidebar-foot"><span className="status-dot"/>SQLite 本地数据库</div>
    </aside>

    <main>
      <header className="topbar"><div><p>{new Intl.DateTimeFormat('zh-CN', { month: 'long', day: 'numeric', weekday: 'long' }).format(new Date())}</p><h1>我的一天</h1></div><div className="top-actions"><span>{day.tasks.filter((task) => task.status === 'completed').length}/{day.tasks.length} 已完成</span><button className="round">•••</button></div></header>
      {worklog.error && <div className="error-banner"><span>{worklog.error}</span><button onClick={worklog.clearError}>关闭</button></div>}

      <section className={`focus-strip ${day.rest ? 'resting' : ''}`}>
        <div className="timer-ring"><strong>{formatSeconds(left)}</strong><span>{day.rest ? (day.rest.status === 'running' ? '休息中' : '休息暂停') : day.focus ? (day.focus.status === 'running' ? '专注中' : '已暂停') : '准备专注'}</span></div>
        <div className="focus-copy"><small>{day.rest ? '当前休息' : '当前专注'}</small><h2>{day.rest ? (day.rest.restKind === 'long' ? '长休息 · 让注意力真正恢复' : '短休息 · 离开屏幕活动一下') : activeTask ? `${activeTask.displayCode} ${activeTask.title}` : selectedTask ? `${selectedTask.displayCode} ${selectedTask.title}` : '先选择一项今日任务'}</h2><p>{day.rest ? '休息不会写入每日回顾；结束后再选择下一项任务。' : day.focus ? '记录会自动关联到当前任务' : '选择任务后，一次点击即可开始专注'}</p>
          {!day.rest && day.tasks.some((task) => !task.parentId && task.status !== 'completed') && <select className="focus-picker" value={day.focus?.taskId ?? selectedTask?.id ?? ''} onChange={(event) => chooseFocusTask(event.target.value)}>{day.tasks.filter((task) => !task.parentId && task.status !== 'completed').map((task) => <option key={task.id} value={task.id}>{task.displayCode} {task.title}</option>)}</select>}
        </div>
        <div className="focus-actions">
          {!day.focus && !day.rest && selectedTask && <button disabled={worklog.busy} className="primary" onClick={() => ignore(worklog.startFocus(selectedTask.id))}>▶ 开始专注</button>}
          {day.focus && <><button disabled={worklog.busy} className="secondary" onClick={() => day.focus?.status === 'running' ? setPauseOpen(true) : ignore(worklog.resumeFocus())}>{day.focus.status === 'running' ? 'Ⅱ 暂停' : '▶ 继续'}</button><button className="text-button" onClick={() => ignore(worklog.completeFocus('early_complete'))}>提前完成</button><button className="text-button danger" onClick={() => ignore(worklog.completeFocus('abandoned'))}>放弃</button></>}
          {day.rest && <><button disabled={worklog.busy} className="secondary" onClick={() => ignore(day.rest?.status === 'running' ? worklog.pauseRest() : worklog.resumeRest())}>{day.rest.status === 'running' ? 'Ⅱ 暂停休息' : '▶ 继续休息'}</button><button className="text-button" onClick={() => ignore(worklog.skipRest())}>跳过休息</button></>}
        </div>
      </section>

      <div className="content-grid">
        <section className="tasks-panel">
          <div className="section-heading"><div><h2>今日安排</h2><p>按重要性与紧急性组织，不让任务列表变得太吵</p></div><TaskForm compact onCreate={createTask}/></div>
          <div className="quadrants">{quadrants.map((quadrant) => {
            const tasks = incompleteFirst(day.tasks.filter((task) => !task.parentId && task.importance === quadrant.importance && task.urgency === quadrant.urgency))
            return <article className="quadrant" key={quadrant.key}>
              <header><div><h3>{quadrant.title}</h3><small>{quadrant.note}</small></div><div className="quadrant-actions"><span>{tasks.length}</span><button type="button" title={`在${quadrant.title}中新建任务`} aria-label={`在${quadrant.title}中新建任务`} onClick={() => setQuickCreateQuadrant(quickCreateQuadrant === quadrant.key ? null : quadrant.key)}>＋</button></div></header>
              {quickCreateQuadrant === quadrant.key && <TaskForm importance={quadrant.importance} urgency={quadrant.urgency} onCreate={createTask}/>}
              <div className="task-list">{tasks.length === 0 && <p className="empty">暂时没有事项</p>}{tasks.map((task) => {
                const children = incompleteFirst(day.tasks.filter((child) => child.parentId === task.id))
                return <div key={task.id} className={`task-card ${task.status === 'completed' ? 'done' : ''} ${selectedTask?.id === task.id ? 'selected' : ''}`} onClick={() => setSelectedTaskId(task.id)}>
                  <button type="button" className="check" aria-label={`${task.status === 'completed' ? '标记为未完成' : '标记为已完成'}：${task.title}`} aria-pressed={task.status === 'completed'} onClick={(event) => { event.stopPropagation(); toggleTask(task) }}>{task.status === 'completed' && <CompletionMark />}</button>
                  <div className="task-body">
                    {editingTaskId === task.id ? <TaskEditForm task={task} onSave={updateTask} onCancel={() => setEditingTaskId(null)}/> : <>
                      <div className="task-title-row"><strong><span>{task.displayCode}</span> {task.title}</strong><button type="button" className="edit-task" onClick={(event) => { event.stopPropagation(); setEditingTaskId(task.id) }}>编辑</button></div>
                      {task.plannedStart && <small>{task.plannedStart}–{task.plannedEnd}</small>}
                    </>}
                    {children.map((child) => editingTaskId === child.id
                      ? <TaskEditForm key={child.id} task={child} onSave={updateTask} onCancel={() => setEditingTaskId(null)}/>
                      : <div className="child" key={child.id}>
                        <button type="button" className="mini-check" aria-label={`${child.status === 'completed' ? '标记为未完成' : '标记为已完成'}：${child.title}`} aria-pressed={child.status === 'completed'} onClick={(event) => { event.stopPropagation(); toggleTask(child) }}>{child.status === 'completed' && <CompletionMark />}</button>
                        <span className={child.status === 'completed' ? 'strike' : ''}>{child.displayCode} {child.title}{child.plannedStart ? ` · ${child.plannedStart}–${child.plannedEnd}` : ''}</span>
                        <button type="button" className="edit-task child-edit" onClick={(event) => { event.stopPropagation(); setEditingTaskId(child.id) }}>编辑</button>
                      </div>)}
                    {childFor === task.id ? <TaskForm parentId={task.id} importance={task.importance} urgency={task.urgency} onCreate={createTask}/> : <button className="add-child" onClick={(event) => { event.stopPropagation(); setChildFor(task.id) }}>＋ 新建子任务</button>}
                  </div>
                </div>
              })}</div>
            </article>
          })}</div>
        </section>

        <aside className="timeline-panel">
          <div className="section-heading"><div><h2>今日记录</h2><p>只展示值得回顾的内容</p></div><span className="live"><i/>{worklog.busy ? '写入中' : '实时'}</span></div>
          <form className="thought-form" onSubmit={addThought}><textarea value={thought} onChange={(event) => setThought(event.target.value)} placeholder="记录一个工作想法…"/><div><select value={entryType} onChange={(event) => setEntryType(event.target.value as EntryType)}>{Object.entries(entryLabels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select><select value={reviewLevel} onChange={(event) => setReviewLevel(event.target.value as ReviewLevel)}>{Object.entries(levelLabels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select><button disabled={worklog.busy} type="submit">记录</button></div></form>
          <div className="timeline">{day.timeline.filter((item) => item.visibility !== 'hidden').length === 0 && <div className="timeline-empty"><b>今天会自然形成一份日记</b><span>完成任务、开始专注或写下想法后，记录会出现在这里。</span></div>}{[...day.timeline].reverse().filter((item) => item.visibility !== 'hidden').map((item) => <div className="event" key={item.id}><time>{formatClock(item.occurredAt)}</time><i/><div><p>{item.title}</p>{item.detail && <small>{item.detail}</small>}</div></div>)}</div>
        </aside>
      </div>
    </main>
    {pauseOpen && <div className="pause-reason-backdrop" onMouseDown={() => setPauseOpen(false)}>
      <form className="pause-reason-dialog" onSubmit={pauseFocus} onMouseDown={(event) => event.stopPropagation()}>
        <header><div><small>FOCUS PAUSE</small><h2>为什么暂停？</h2></div><button type="button" onClick={() => setPauseOpen(false)}>×</button></header>
        <p>暂停原因会写入本轮番茄钟时间线，帮助回顾注意力被什么打断。</p>
        <textarea autoFocus required value={pauseReason} onChange={(event) => setPauseReason(event.target.value)} placeholder="例如：临时回复重要消息、等待资料下载、注意力涣散…"/>
        <footer><button type="button" className="cancel" onClick={() => setPauseOpen(false)}>取消</button><button type="submit" disabled={!pauseReason.trim() || worklog.busy}>确认暂停</button></footer>
      </form>
    </div>}
  </div>
}

interface TaskFormProps {
  compact?: boolean
  parentId?: string | null
  importance?: Importance
  urgency?: Urgency
  onCreate: (title: string, importance: Importance, urgency: Urgency, parentId: string | null, start: string | null, end: string | null) => void
}

function TaskEditForm({ task, onSave, onCancel }: { task: DayTask; onSave: (task: DayTask, title: string, start: string | null, end: string | null) => void; onCancel: () => void }) {
  const [title, setTitle] = useState(task.title)
  const [timed, setTimed] = useState(Boolean(task.plannedStart && task.plannedEnd))
  const [start, setStart] = useState(task.plannedStart ?? '09:00')
  const [end, setEnd] = useState(task.plannedEnd ?? '10:00')
  return <form className="task-edit-form" onSubmit={(event) => { event.preventDefault(); if (title.trim()) onSave(task, title.trim(), timed ? start : null, timed ? end : null) }} onClick={(event) => event.stopPropagation()}>
    <input autoFocus value={title} onChange={(event) => setTitle(event.target.value)} aria-label={`修改${task.displayCode}内容`}/>
    <label><input type="checkbox" checked={timed} onChange={(event) => setTimed(event.target.checked)}/>安排时间</label>
    {timed && <span className="time-range"><input type="time" value={start} onChange={(event) => setStart(event.target.value)}/><b>–</b><input type="time" value={end} onChange={(event) => setEnd(event.target.value)}/></span>}
    <div><button type="submit">保存</button><button type="button" className="cancel" onClick={onCancel}>取消</button></div>
  </form>
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
