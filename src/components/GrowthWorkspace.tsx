import { useEffect, useState, type FormEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { GrowthCategories, CategoryManager, CategoryPicker } from './GrowthCategories'
import GoalActionEditor from './GoalActionEditor'
import type { GoalAction } from '../domain/planning'

interface Habit { id: string; title: string; startDate: string; prerequisiteIds: string[] }
interface HabitReviewItem { habitId: string; title: string; prerequisiteIds: string[]; prerequisiteTitles: string[]; rawCompleted: boolean; effectiveCompleted: boolean; blockedByTitles: string[] }
interface HabitReview { reviewDate: string; finalized: boolean; items: HabitReviewItem[] }
interface GoalPhase { id: string; title: string; startDate: string; endDate: string; brainstormMd: string; actions: GoalAction[] }
interface LongTermGoal { id: string; title: string; descriptionMd: string; cycleDays: number; startDate: string; status: string; phases: GoalPhase[]; progressPercent: number; trophy: 'bronze' | 'silver' | 'gold' | null }

const desktop = '__TAURI_INTERNALS__' in window
const dateText = (date: Date) => [date.getFullYear(), String(date.getMonth() + 1).padStart(2, '0'), String(date.getDate()).padStart(2, '0')].join('-')
const today = () => dateText(new Date())
const yesterday = () => { const date = new Date(); date.setDate(date.getDate() - 1); return dateText(date) }
const trophyLabel = { bronze: '🥉 铜杯', silver: '🥈 银杯', gold: '🥇 金杯' } as const

export default function GrowthWorkspace() {
  const [open, setOpen] = useState(false)
  const [tab, setTab] = useState<'habits' | 'goals' | 'categories'>('habits')
  const [habits, setHabits] = useState<Habit[]>([])
  const [review, setReview] = useState<HabitReview | null>(null)
  const [goals, setGoals] = useState<LongTermGoal[]>([])
  const [checked, setChecked] = useState<Set<string>>(new Set())
  const [habitTitle, setHabitTitle] = useState('')
  const [prerequisites, setPrerequisites] = useState<Set<string>>(new Set())
  const [goalTitle, setGoalTitle] = useState('')
  const [goalDescription, setGoalDescription] = useState('')
  const [cycleDays, setCycleDays] = useState(30)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')

  async function loadAll() {
    if (!desktop) return
    setBusy(true); setError('')
    try {
      const [habitResult, reviewResult, goalResult] = await Promise.all([
        invoke<Habit[]>('list_habits'),
        invoke<HabitReview>('get_habit_review', { reviewDate: yesterday() }),
        invoke<LongTermGoal[]>('list_long_term_goals'),
      ])
      setHabits(habitResult); setReview(reviewResult); setGoals(goalResult)
      setChecked(new Set(reviewResult.items.filter((item) => item.rawCompleted).map((item) => item.habitId)))
    } catch (reason) { setError(String(reason)) }
    finally { setBusy(false) }
  }

  useEffect(() => {
    const show = () => { setOpen(true); void loadAll() }
    const showCategories = () => { setTab('categories'); show() }
    window.addEventListener('worklog:open-growth', show)
    window.addEventListener('worklog:open-categories', showCategories)
    return () => { window.removeEventListener('worklog:open-growth', show); window.removeEventListener('worklog:open-categories', showCategories) }
  }, [])

  async function createHabit(event: FormEvent) {
    event.preventDefault(); if (!habitTitle.trim()) return
    setBusy(true); setError('')
    try {
      setHabits(await invoke<Habit[]>('create_habit', { input: { title: habitTitle.trim(), startDate: today(), prerequisiteIds: [...prerequisites] } }))
      setHabitTitle(''); setPrerequisites(new Set()); setReview(await invoke<HabitReview>('get_habit_review', { reviewDate: yesterday() }))
    } catch (reason) { setError(String(reason)) }
    finally { setBusy(false) }
  }

  async function archiveHabit(id: string) {
    setBusy(true); setError('')
    try { setHabits(await invoke<Habit[]>('archive_habit', { habitId: id })) }
    catch (reason) { setError(String(reason)) }
    finally { setBusy(false) }
  }

  async function finalizeReview() {
    if (!review) return
    setBusy(true); setError('')
    try {
      const result = await invoke<HabitReview>('complete_habit_review', { input: { reviewDate: review.reviewDate, completedHabitIds: [...checked] } })
      setReview(result)
      window.dispatchEvent(new Event('worklog:reload'))
    } catch (reason) { setError(String(reason)) }
    finally { setBusy(false) }
  }

  async function createGoal(event: FormEvent) {
    event.preventDefault(); if (!goalTitle.trim()) return
    setBusy(true); setError('')
    try {
      setGoals(await invoke<LongTermGoal[]>('create_long_term_goal', { input: { title: goalTitle.trim(), descriptionMd: goalDescription.trim(), cycleDays, startDate: today() } }))
      setGoalTitle(''); setGoalDescription(''); setCycleDays(30)
    } catch (reason) { setError(String(reason)) }
    finally { setBusy(false) }
  }

  if (!open) return null
  return <div className="growth-backdrop" onMouseDown={() => setOpen(false)}>
    <section className="growth-workspace" role="dialog" aria-modal="true" aria-label="成长系统" onMouseDown={(event) => event.stopPropagation()}>
      <header><div><small>PERSONAL GROWTH</small><h2>成长</h2><p>习惯打卡与长期目标</p></div><button onClick={() => setOpen(false)}>×</button></header>
      <nav><button className={tab === 'habits' ? 'active' : ''} onClick={() => setTab('habits')}>昨日打卡</button><button className={tab === 'goals' ? 'active' : ''} onClick={() => setTab('goals')}>长期目标</button><button className={tab === 'categories' ? 'active' : ''} onClick={() => setTab('categories')}>分类与分享</button></nav>
      <GrowthCategories>
      {error && <p className="growth-error">{error}</p>}
      {!desktop && <p className="growth-empty">请在桌面版管理打卡与目标。</p>}

      {desktop && tab === 'habits' && <div className="growth-content habit-layout">
        <section className="review-card">
          <div className="growth-heading"><div><small>昨日 · {review?.reviewDate ?? yesterday()}</small><h3>回顾昨日</h3></div>{review?.finalized && <span>已保存</span>}</div>
          {!review?.items.length && <p className="growth-empty">昨日没有打卡项。新建打卡从明天开始回顾。</p>}
          <div className="habit-review-list">{review?.items.map((item) => <label key={item.habitId} className={review.finalized && item.rawCompleted && !item.effectiveCompleted ? 'dependency-failed' : ''}>
            <input type="checkbox" disabled={review.finalized} checked={review.finalized ? item.rawCompleted : checked.has(item.habitId)} onChange={(event) => { const next = new Set(checked); if (event.target.checked) next.add(item.habitId); else next.delete(item.habitId); setChecked(next) }}/>
            <span><b>{item.title}</b>{item.prerequisiteTitles.length > 0 && <small>前置：{item.prerequisiteTitles.join('、')}</small>}{review.finalized && item.rawCompleted && !item.effectiveCompleted && <em>已做到；“{item.blockedByTitles.join('、')}”尚未完成</em>}</span>
            {review.finalized && <strong>{item.effectiveCompleted ? '已完成' : '未完成'}</strong>}
          </label>)}</div>
          {review && !review.finalized && review.items.length > 0 && <button className="growth-primary" disabled={busy} onClick={() => void finalizeReview()}>保存昨日打卡</button>}
        </section>

        <aside className="habit-settings">
          <div className="growth-heading"><div><small>HABITS</small><h3>设置打卡项</h3></div></div>
          <form onSubmit={createHabit}><input value={habitTitle} onChange={(event) => setHabitTitle(event.target.value)} placeholder="例如：22:00 前睡觉"/>
            {habits.length > 0 && <fieldset><legend>选择前置打卡</legend>{habits.map((habit) => <label key={habit.id}><input type="checkbox" checked={prerequisites.has(habit.id)} onChange={(event) => { const next = new Set(prerequisites); if (event.target.checked) next.add(habit.id); else next.delete(habit.id); setPrerequisites(next) }}/>{habit.title}</label>)}</fieldset>}
            <button disabled={busy || !habitTitle.trim()}>新建打卡</button>
          </form>
          <div className="habit-definitions">{habits.map((habit) => <div key={habit.id}><span><b>{habit.title}</b><small>{habit.prerequisiteIds.length ? `有 ${habit.prerequisiteIds.length} 个前置项` : '独立打卡'}</small><CategoryPicker entityId={habit.id} kind="habit"/></span><button disabled={busy} onClick={() => void archiveHabit(habit.id)}>停用</button></div>)}</div>
        </aside>
      </div>}

      {desktop && tab === 'goals' && <div className="growth-content goals-layout">
        <form className="new-goal-form" onSubmit={createGoal}><div><small>NEW LONG-TERM GOAL</small><h3>开始一个长期目标</h3></div><input value={goalTitle} onChange={(event) => setGoalTitle(event.target.value)} placeholder="例如：一个月内看懂电气原理图"/><textarea value={goalDescription} onChange={(event) => setGoalDescription(event.target.value)} placeholder="写下为什么想完成它，以及怎样算真正完成"/><label>循环周期<input type="number" min="1" max="3660" value={cycleDays} onChange={(event) => setCycleDays(Number(event.target.value))}/><span>天</span></label><button disabled={busy || !goalTitle.trim()}>创建目标</button></form>
        <div className="goal-list">{!goals.length && <p className="growth-empty">暂无长期目标。</p>}{goals.map((goal) => <GoalCard key={goal.id} goal={goal} disabled={busy} onChange={setGoals} onError={setError}/>)}</div>
      </div>}
      {desktop && tab === 'categories' && <CategoryManager/>}
      </GrowthCategories>
    </section>
  </div>
}

function GoalCard({ goal, disabled, onChange, onError }: { goal: LongTermGoal; disabled: boolean; onChange: (goals: LongTermGoal[]) => void; onError: (error: string) => void }) {
  const [addingPhase, setAddingPhase] = useState(false)
  const [phaseTitle, setPhaseTitle] = useState('')
  const [phaseStart, setPhaseStart] = useState(today)
  const [phaseEnd, setPhaseEnd] = useState(() => { const date = new Date(); date.setDate(date.getDate() + 6); return dateText(date) })
  const [note, setNote] = useState('')

  async function createPhase(event: FormEvent) {
    event.preventDefault()
    try { onChange(await invoke<LongTermGoal[]>('create_goal_phase', { input: { goalId: goal.id, title: phaseTitle, startDate: phaseStart, endDate: phaseEnd, brainstormMd: note } })); setAddingPhase(false); setPhaseTitle(''); setNote('') }
    catch (reason) { onError(String(reason)) }
  }

  return <article className="goal-card">
    <CategoryPicker entityId={goal.id} kind="goal"/>
    <header><div><small>{goal.cycleDays} 天周期 · 始于 {goal.startDate}</small><h3>{goal.title}</h3><p>{goal.descriptionMd}</p></div><div className={`goal-trophy ${goal.trophy ?? ''}`}>{goal.trophy ? trophyLabel[goal.trophy] : `${goal.progressPercent}%`}</div></header>
    <div className="goal-progress"><i style={{ width: `${Math.min(goal.progressPercent, 140) / 1.4}%` }}/><span>{goal.progressPercent}%</span></div>
    <div className="phase-list">{goal.phases.map((phase) => <PhaseCard key={phase.id} phase={phase} disabled={disabled} onChange={onChange} onError={onError}/>)}</div>
    {addingPhase ? <form className="phase-form" onSubmit={createPhase}><input value={phaseTitle} onChange={(event) => setPhaseTitle(event.target.value)} placeholder="当前阶段，例如：学会电工识图"/><div><input type="date" value={phaseStart} onChange={(event) => setPhaseStart(event.target.value)}/><span>至</span><input type="date" value={phaseEnd} onChange={(event) => setPhaseEnd(event.target.value)}/></div><textarea value={note} onChange={(event) => setNote(event.target.value)} placeholder="自由写下这一阶段的零碎想法与疑问…"/><footer><button>保存阶段</button><button type="button" onClick={() => setAddingPhase(false)}>取消</button></footer></form> : <button className="text-add" onClick={() => setAddingPhase(true)}>＋ 规划最近阶段</button>}
  </article>
}

function PhaseCard({ phase, disabled, onChange, onError }: { phase: GoalPhase; disabled: boolean; onChange: (goals: LongTermGoal[]) => void; onError: (error: string) => void }) {
  const [note, setNote] = useState(phase.brainstormMd)
  const [title, setTitle] = useState('')
  const [kind, setKind] = useState<'one_off' | 'repeating'>('one_off')
  const [required, setRequired] = useState(true)
  const [targetCount, setTargetCount] = useState(1)

  async function saveNote() { try { onChange(await invoke<LongTermGoal[]>('save_goal_phase_note', { input: { phaseId: phase.id, brainstormMd: note } })) } catch (reason) { onError(String(reason)) } }
  async function createAction(event: FormEvent) { event.preventDefault(); try { onChange(await invoke<LongTermGoal[]>('create_goal_action', { input: { phaseId: phase.id, title, actionKind: kind, required, targetCount } })); setTitle(''); setTargetCount(1) } catch (reason) { onError(String(reason)) } }

  return <section className="phase-card"><header><div><small>{phase.startDate} — {phase.endDate}</small><h4>{phase.title}</h4></div></header>
    <div className="brainstorm"><textarea value={note} onChange={(event) => setNote(event.target.value)} placeholder="写下想法、问题或学习笔记…"/><button disabled={disabled || note === phase.brainstormMd} onClick={() => void saveNote()}>保存随想</button></div>
    <div className="goal-actions">{phase.actions.map(action => <GoalActionEditor<LongTermGoal[]> key={action.id} action={action} start={phase.startDate} end={phase.endDate} onChange={onChange} onError={onError}/>)}</div>
    <form className="action-form" onSubmit={createAction}><input value={title} onChange={(event) => setTitle(event.target.value)} placeholder="把随想整理成可执行事项"/><select value={kind} onChange={(event) => setKind(event.target.value as typeof kind)}><option value="one_off">一次性事件</option><option value="repeating">重复事件</option></select><select value={required ? 'required' : 'optional'} onChange={(event) => setRequired(event.target.value === 'required')}><option value="required">必须</option><option value="optional">附加</option></select><input type="number" min="1" max="3660" value={targetCount} onChange={(event) => setTargetCount(Number(event.target.value))}/><button disabled={!title.trim()}>添加</button></form>
  </section>
}
