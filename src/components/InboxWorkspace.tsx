import { useEffect, useState, type FormEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { localDate, type DayState } from '../domain/model'
import { groupHistoricalTasks, historicalStatusLabel, type HistoricalUnfinishedTask } from '../domain/history'

interface InboxTask { taskId: string; title: string; groupId: string; parentTaskId: string | null }
type InboxTab = 'inbox' | 'history'
const desktop = '__TAURI_INTERNALS__' in window

export default function InboxWorkspace() {
  const [open, setOpen] = useState(false)
  const [tab, setTab] = useState<InboxTab>('inbox')
  const [items, setItems] = useState<InboxTask[]>([])
  const [historical, setHistorical] = useState<HistoricalUnfinishedTask[]>([])
  const [day, setDay] = useState<DayState | null>(null)
  const [title, setTitle] = useState('')
  const [date, setDate] = useState(localDate)
  const [selected, setSelected] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const historyGroups = groupHistoricalTasks(historical)

  async function refresh() {
    const today = localDate()
    const [inbox, currentDay, unfinished] = await Promise.all([
      invoke<InboxTask[]>('list_inbox'),
      invoke<DayState>('get_day_snapshot', { workDate: today }),
      invoke<HistoricalUnfinishedTask[]>('list_historical_unfinished', { beforeDate: today }),
    ])
    setItems(inbox); setDay(currentDay); setHistorical(unfinished)
  }
  async function run(operation: () => Promise<unknown>) {
    setBusy(true); setError('')
    try { await operation(); await refresh(); window.dispatchEvent(new Event('worklog:reload')) }
    catch (reason) { setError(String(reason)) }
    finally { setBusy(false) }
  }
  useEffect(() => {
    const show = (event: Event) => {
      const selectedTask = (event as CustomEvent<string>).detail || ''
      setOpen(true); setDate(localDate()); setSelected(selectedTask); setTab(selectedTask ? 'inbox' : 'history')
      if (desktop) void run(async () => {})
    }
    window.addEventListener('worklog:open-inbox', show)
    return () => window.removeEventListener('worklog:open-inbox', show)
  }, [])
  function create(event: FormEvent) {
    event.preventDefault()
    if (!title.trim()) return
    void run(async () => { await invoke('create_inbox_task', { title: title.trim() }); setTitle('') })
  }
  if (!open) return null
  return <div className="growth-backdrop" onMouseDown={() => { if (!busy) setOpen(false) }}>
    <section className="growth-workspace inbox-workspace" role="dialog" aria-modal="true" aria-label="待办与历史未完成" onMouseDown={event => event.stopPropagation()}>
      <header><div><small>INBOX & HISTORY</small><h2>待办与历史未完成</h2><p>收集尚未定期的任务，找回遗漏的历史安排</p></div><button disabled={busy} aria-label="关闭待办与历史未完成" onClick={() => setOpen(false)}>×</button></header>
      <div className="inbox-tabs" role="tablist" aria-label="任务来源">
        <button type="button" role="tab" aria-selected={tab === 'history'} className={tab === 'history' ? 'active' : ''} onClick={() => setTab('history')}>历史未完成 <span>{historyGroups.length}</span></button>
        <button type="button" role="tab" aria-selected={tab === 'inbox'} className={tab === 'inbox' ? 'active' : ''} onClick={() => setTab('inbox')}>待办箱 <span>{items.filter(item => !item.parentTaskId).length}</span></button>
      </div>
      {error && <p className="growth-error" role="alert">{error}</p>}
      {!desktop ? <p className="growth-empty">请在桌面版使用待办与历史任务。</p> : <div className="inbox-content">
        {tab === 'inbox' ? <>
          <form onSubmit={create}><label htmlFor="inbox-title">暂时没有日期的事</label><div><input id="inbox-title" disabled={busy} value={title} onChange={e => setTitle(e.target.value)} placeholder="例如：整理一个电气识图资料库"/><button disabled={busy || !title.trim()}>收入待办箱</button></div></form>
          <div className="inbox-capture"><label htmlFor="inbox-existing">从今日安排收纳</label><div><select id="inbox-existing" disabled={busy} value={selected} onChange={e => setSelected(e.target.value)}><option value="">选择现有任务…</option>{day?.tasks.map(task => <option key={task.id} value={task.id}>{task.displayCode} {task.title}</option>)}</select><button disabled={busy || !selected} onClick={() => void run(async () => { await invoke('move_task_to_inbox', { instanceId: selected, workDate: localDate() }); setSelected('') })}>收纳所选任务</button></div><small>收纳父任务时，子任务会一起移入。</small></div>
          <label className="inbox-date">准备安排到<input type="date" disabled={busy} min={localDate()} value={date} onChange={e => setDate(e.target.value)}/></label>
          {!items.length && <p className="growth-empty">待办箱暂无任务。</p>}
          {items.filter(item => !item.parentTaskId).map(root => <article className="inbox-item" key={root.taskId}><div><h3>{root.title}</h3>{items.filter(child => child.parentTaskId === root.taskId && child.groupId === root.groupId).map(child => <p key={child.taskId}>↳ {child.title}</p>)}</div><button disabled={busy || !date || date < localDate()} onClick={() => void run(() => invoke('schedule_inbox_task', { groupId: root.groupId, workDate: date }))}>安排到 {date === localDate() ? '今天' : date}</button></article>)}
        </> : <>
          <div className="history-reschedule-heading">
            <div><h3>历史未完成任务</h3><p>只显示尚未处理、且没有较新安排的任务。重新安排不会删除原日期记录。</p></div>
            <label className="inbox-date">重新安排到<input type="date" disabled={busy} min={localDate()} value={date} onChange={e => setDate(e.target.value)}/></label>
          </div>
          {!historyGroups.length && <p className="growth-empty">没有遗漏的历史未完成任务。</p>}
          <div className="historical-list">{historyGroups.map(group => <article className="historical-item" key={group.root.instanceId}>
            <div className="historical-copy">
              <div className="historical-meta"><time>{group.root.workDate}</time><span>{historicalStatusLabel(group.root.status)}</span><span>{group.root.displayCode}</span></div>
              <h3>{group.root.title}</h3>
              {group.children.map(child => <p key={child.instanceId}>↳ {child.displayCode} {child.title} <small>{historicalStatusLabel(child.status)}</small></p>)}
              {group.blockedReason && <small className="historical-blocked">{group.blockedReason}</small>}
            </div>
            <button disabled={busy || !date || date < localDate() || !group.reschedulable} onClick={() => void run(() => invoke('reschedule_historical_task', { sourceInstanceId: group.root.instanceId, targetDate: date }))}>重新安排到 {date === localDate() ? '今天' : date}</button>
          </article>)}</div>
        </>}
      </div>}
    </section>
  </div>
}
