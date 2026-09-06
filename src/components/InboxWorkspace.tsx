import { useEffect, useState, type FormEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { localDate, type DayState } from '../domain/model'

interface InboxTask { taskId: string; title: string; groupId: string; parentTaskId: string | null }
const desktop = '__TAURI_INTERNALS__' in window

export default function InboxWorkspace() {
  const [open, setOpen] = useState(false)
  const [items, setItems] = useState<InboxTask[]>([])
  const [day, setDay] = useState<DayState | null>(null)
  const [title, setTitle] = useState('')
  const [date, setDate] = useState(localDate)
  const [selected, setSelected] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  async function refresh() {
    const [inbox, today] = await Promise.all([invoke<InboxTask[]>('list_inbox'), invoke<DayState>('get_day_snapshot', { workDate: localDate() })])
    setItems(inbox); setDay(today)
  }
  async function run(operation: () => Promise<unknown>) {
    setBusy(true); setError('')
    try { await operation(); await refresh(); window.dispatchEvent(new Event('worklog:reload')) }
    catch (reason) { setError(String(reason)) }
    finally { setBusy(false) }
  }
  useEffect(() => {
    const show = (event: Event) => {
      setOpen(true); setDate(localDate()); setSelected((event as CustomEvent<string>).detail || '')
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
    <section className="growth-workspace inbox-workspace" role="dialog" aria-modal="true" aria-label="待办箱" onMouseDown={event => event.stopPropagation()}>
      <header><div><small>INBOX</small><h2>待办箱</h2><p>尚未安排日期的任务</p></div><button disabled={busy} aria-label="关闭待办箱" onClick={() => setOpen(false)}>×</button></header>
      {error && <p className="growth-error" role="alert">{error}</p>}
      {!desktop ? <p className="growth-empty">请在桌面版使用待办箱。</p> : <div className="inbox-content">
        <form onSubmit={create}><label htmlFor="inbox-title">暂时没有日期的事</label><div><input id="inbox-title" disabled={busy} value={title} onChange={e => setTitle(e.target.value)} placeholder="例如：整理一个电气识图资料库"/><button disabled={busy || !title.trim()}>收入待办箱</button></div></form>
        <div className="inbox-capture"><label htmlFor="inbox-existing">从今日安排收纳</label><div><select id="inbox-existing" disabled={busy} value={selected} onChange={e => setSelected(e.target.value)}><option value="">选择现有任务…</option>{day?.tasks.map(task => <option key={task.id} value={task.id}>{task.displayCode} {task.title}</option>)}</select><button disabled={busy || !selected} onClick={() => void run(async () => { await invoke('move_task_to_inbox', { instanceId: selected, workDate: localDate() }); setSelected('') })}>收纳所选任务</button></div><small>收纳父任务时，子任务会一起移入。</small></div>
        <label className="inbox-date">准备安排到<input type="date" disabled={busy} min={localDate()} value={date} onChange={e => setDate(e.target.value)}/></label>
        {!items.length && <p className="growth-empty">待办箱暂无任务。</p>}
        {items.filter(item => !item.parentTaskId).map(root => <article className="inbox-item" key={root.taskId}><div><h3>{root.title}</h3>{items.filter(child => child.parentTaskId === root.taskId && child.groupId === root.groupId).map(child => <p key={child.taskId}>↳ {child.title}</p>)}</div><button disabled={busy || !date || date < localDate()} onClick={() => void run(() => invoke('schedule_inbox_task', { groupId: root.groupId, workDate: date }))}>安排到 {date === localDate() ? '今天' : date}</button></article>)}
      </div>}
    </section>
  </div>
}
