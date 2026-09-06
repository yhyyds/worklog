import { useState, type FormEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { dateRange, type GoalAction } from '../domain/planning'
import { taskLabels } from '../domain/report'

export default function GoalActionEditor<T>({ action, start, end, onChange, onError }: { action: GoalAction; start: string; end: string; onChange: (goals: T) => void; onError: (error: string) => void }) {
  const [editing, setEditing] = useState(false), [deleting, setDeleting] = useState(false), [busy, setBusy] = useState(false)
  const [title, setTitle] = useState(action.title), [kind, setKind] = useState(action.actionKind), [required, setRequired] = useState(action.required)
  const [target, setTarget] = useState(action.targetCount), [importance, setImportance] = useState(action.importance), [urgency, setUrgency] = useState(action.urgency)
  const [dates, setDates] = useState(action.occurrences.map(o => o.date))
  const today = dateRange(new Date().toLocaleDateString('sv-SE'), new Date().toLocaleDateString('sv-SE'))[0]
  const [date, setDate] = useState(start > today ? start : today)
  async function run(command: string, args: Record<string, unknown>) {
    setBusy(true); onError('')
    try { onChange(await invoke<T>(command, args)); setEditing(false); setDeleting(false); window.dispatchEvent(new Event('worklog:reload')) }
    catch (e) { onError(String(e)) }
    finally { setBusy(false) }
  }
  function addDates(values: string[]) { const next = [...new Set([...dates, ...values])].sort(); setDates(next); setTarget(Math.max(target, action.manualCompletedCount + next.length)) }
  function edit() { setTitle(action.title); setKind(action.actionKind); setRequired(action.required); setTarget(action.targetCount); setImportance(action.importance); setUrgency(action.urgency); setDates(action.occurrences.map(o => o.date)); setEditing(true) }
  function save(e: FormEvent) { e.preventDefault(); void run('save_goal_action_plan', { input: { actionId: action.id, title, actionKind: kind, required, targetCount: target, importance, urgency, dates } }) }
  return <article className="planned-action">
    <header><div><b>{action.title}</b><small>{action.actionKind === 'repeating' ? '重复' : '一次性'} · {action.required ? '必须' : '附加'} · {action.importance === 'important' ? '重要' : '次要'} · {action.urgency === 'urgent' ? '紧急' : '稍缓'}</small></div><strong>{action.completedCount}/{action.targetCount}</strong></header>
    <div className="planned-dates">{action.occurrences.map(o => <span key={o.id} className={o.status === 'completed' ? 'done' : ''}>{o.date.slice(5)} · {taskLabels[o.status] ?? o.status}</span>)}</div>
    <footer><button disabled={busy} onClick={edit}>编辑与排期</button><button disabled={busy} onClick={() => setDeleting(true)}>删除</button>{!action.tracked && <><button aria-label="减少完成次数" disabled={busy || action.completedCount <= 0} onClick={() => void run('set_goal_action_progress', { input: { actionId: action.id, completedCount: action.completedCount - 1 } })}>−</button><button aria-label="增加完成次数" disabled={busy || action.completedCount >= action.targetCount} onClick={() => void run('set_goal_action_progress', { input: { actionId: action.id, completedCount: action.completedCount + 1 } })}>＋</button></>}</footer>
    {deleting && <div className="delete-confirm"><p>删除“{action.title}”？尚未开始的今天及未来安排一并移除，已有执行记录保留。该任务不再计入目标进度。</p><button disabled={busy} onClick={() => void run('delete_goal_action', { actionId: action.id })}>确认删除</button><button disabled={busy} onClick={() => setDeleting(false)}>取消</button></div>}
    {editing && <form className="plan-editor" onSubmit={save}><label>任务名称<input required value={title} onChange={e => setTitle(e.target.value)}/></label><div className="plan-fields"><label>类型<select value={kind} onChange={e => setKind(e.target.value as typeof kind)}><option value="one_off">一次性</option><option value="repeating">重复</option></select></label><label>要求<select value={required ? 'required' : 'optional'} onChange={e => setRequired(e.target.value === 'required')}><option value="required">必须</option><option value="optional">附加</option></select></label><label>计划次数<input type="number" min={Math.max(1, action.manualCompletedCount + dates.length)} max={3660} value={target} onChange={e => setTarget(Number(e.target.value))}/></label><label>重要程度<select value={importance} onChange={e => setImportance(e.target.value)}><option value="important">重要</option><option value="secondary">次要</option></select></label><label>紧急程度<select value={urgency} onChange={e => setUrgency(e.target.value)}><option value="urgent">紧急</option><option value="relaxed">稍缓</option></select></label></div>
      <fieldset><legend>安排到每天</legend><div className="plan-fields"><input aria-label="执行日期" type="date" min={start > today ? start : today} max={end} value={date} onChange={e => setDate(e.target.value)}/><button type="button" disabled={!date || date < today || date < start || date > end} onClick={() => addDates([date])}>添加日期</button>{kind === 'repeating' && <><button type="button" onClick={() => addDates(dateRange(start > today ? start : today, end))}>阶段内每天</button><button type="button" onClick={() => addDates(dateRange(start > today ? start : today, end, true))}>阶段内工作日</button></>}</div><div className="planned-dates">{dates.map(d => { const old = action.occurrences.find(o => o.date === d); const locked = !!old && (d < today || old.status !== 'not_started'); return <button key={d} type="button" disabled={locked} title={locked ? '已有记录，保留原安排' : '移除此日期'} onClick={() => setDates(dates.filter(value => value !== d))}>{d} {locked ? '· 已记录' : '×'}</button> })}</div></fieldset>
      <small>完成情况与“我的一天”同步。重复任务不顺延；一次性任务可在日终顺延。已有手动完成 {action.manualCompletedCount} 次。</small>
      <footer><button disabled={busy || !title.trim()}>保存任务与安排</button><button type="button" disabled={busy} onClick={() => setEditing(false)}>取消</button></footer>
    </form>}
  </article>
}
