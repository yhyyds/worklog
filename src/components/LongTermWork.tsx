import { useEffect, useMemo, useState, type FormEvent } from 'react'
import { groupLongTermItems, WORKLOAD_LABELS, type LongTermItem, type SaveLongTermItem, type Workload } from '../domain/longTerm'
import { createGateway } from '../infrastructure/createGateway'

export function WorkloadSelect({ value, onChange }: { value: Workload | null; onChange: (value: Workload | null) => void }) {
  return <label className="workload-select">
    <span>预估工作量</span>
    <select value={value ?? ''} onChange={(event) => onChange(event.target.value as Workload || null)}>
      <option value="">未估算</option>
      {Object.entries(WORKLOAD_LABELS).map(([key, label]) => <option key={key} value={key}>{label}</option>)}
    </select>
    <small>超过 3 小时，请拆成两个或更多子任务。</small>
  </label>
}

export default function LongTermWork({ onItemsChange }: { onItemsChange: (items: LongTermItem[]) => void }) {
  const gateway = useMemo(createGateway, [])
  const [items, setItems] = useState<LongTermItem[]>([])
  const groups = useMemo(() => groupLongTermItems(items), [items])
  const [expanded, setExpanded] = useState<string | null>(null)
  const [edit, setEdit] = useState<SaveLongTermItem | null>(null)
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const busy = loading || saving
  useEffect(() => {
    let active = true
    let request = 0
    const reload = async () => {
      const id = ++request
      setLoading(true)
      setError(null)
      setEdit(null)
      try {
        const rows = await gateway.listLongTermWork()
        if (active && request === id) setItems(rows)
      } catch (reason) {
        if (active && request === id) setError(String(reason))
      } finally {
        if (active && request === id) setLoading(false)
      }
    }
    void reload()
    window.addEventListener('worklog:storage-changed', reload)
    window.addEventListener('worklog:long-term-changed', reload)
    return () => {
      active = false
      window.removeEventListener('worklog:storage-changed', reload)
      window.removeEventListener('worklog:long-term-changed', reload)
    }
  }, [gateway])
  useEffect(() => { onItemsChange(items) }, [items, onItemsChange])

  function start(parentId: string | null) {
    setEdit({ id: null, parentId, title: '', workload: null, completed: false })
  }

  async function save(input: SaveLongTermItem, closeForm: boolean) {
    setSaving(true)
    setError(null)
    try {
      setItems(await gateway.saveLongTermWork(input))
      if (closeForm) setEdit(null)
      window.dispatchEvent(new Event('worklog:reload'))
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason))
    } finally {
      setSaving(false)
    }
  }

  function toggle(item: LongTermItem) {
    void save({ ...item, completed: !item.completed }, false)
  }

  function submit(event: FormEvent) {
    event.preventDefault()
    if (edit?.title.trim() && !busy) void save(edit, true)
  }

  const editor = edit && <form className="long-work-form" onSubmit={submit}>
    <input autoFocus aria-label={edit.parentId ? '子事项内容' : '长期任务内容'} maxLength={200} value={edit.title}
      onChange={(event) => setEdit({ ...edit, title: event.target.value })}
      placeholder={edit.parentId ? '例如：整理第一轮测试数据' : '例如：完成设备选型与校核'}/>
    {edit.parentId !== null && <WorkloadSelect value={edit.workload} onChange={(workload) => setEdit({ ...edit, workload })}/>}
    <div className="long-work-actions">
      <button type="submit" className="primary small" disabled={busy || !edit.title.trim()}>保存</button>
      <button type="button" onClick={() => setEdit(null)} disabled={busy}>取消</button>
    </div>
  </form>

  return <section className="long-work" aria-labelledby="long-work-title">
    <header className="long-work-heading">
      <div><h3 id="long-work-title">长期工作任务</h3><p>跨天保留，展开主任务查看子事项</p></div>
      <button type="button" className="primary small" disabled={busy || edit !== null} onClick={() => start(null)}>＋ 新增长期任务</button>
    </header>
    {error && <p role="alert" className="long-work-error">{error}</p>}
    {edit?.id === null && edit.parentId === null && editor}
    {groups.length === 0 && !edit && !error && <p className="long-work-empty">{loading ? '正在读取…' : '暂时没有长期任务'}</p>}
    <div className="long-work-list">
      {groups.map(({ item, children, completedChildren }) => {
        const open = expanded === item.id
        const childId = `long-work-${item.id}`
        return <article key={item.id}>
          <div className="long-work-row">
            <button type="button" className="long-work-disclosure" aria-expanded={open} aria-controls={childId}
              disabled={saving || (edit?.parentId != null && edit.parentId === expanded)}
              onClick={() => setExpanded(open ? null : item.id)}>
              <span aria-hidden="true">{open ? '▾' : '▸'}</span>
              <strong className={item.completed ? 'strike' : ''}>{item.title}</strong>
              <small>{completedChildren}/{children.length} 子事项</small>
            </button>
            <button type="button" disabled={busy || edit !== null} onClick={() => setEdit(item)} aria-label={`编辑长期任务：${item.title}`}>编辑</button>
            <button type="button" disabled={busy || edit !== null} onClick={() => toggle(item)} aria-label={`${item.completed ? '重开' : '完成'}长期任务：${item.title}`}>{item.completed ? '重开' : '完成'}</button>
          </div>
          {edit?.id === item.id && editor}
          <div id={childId} hidden={!open} className="long-work-children">
            {open && <>
              {children.map((child) => edit?.id === child.id ? <div key={child.id}>{editor}</div> :
                <div className="long-work-child long-work-row" key={child.id}>
                  <input type="checkbox" checked={child.completed} disabled={busy || edit !== null}
                    aria-label={`完成子事项：${child.title}`} onChange={() => toggle(child)}/>
                  <span className={child.completed ? 'strike' : ''}>{child.title}</span>
                  {child.workload && <small className="workload-badge">{WORKLOAD_LABELS[child.workload]}</small>}
                  <button type="button" disabled={busy || edit !== null} onClick={() => setEdit(child)} aria-label={`编辑子事项：${child.title}`}>编辑</button>
                </div>)}
              {edit?.parentId === item.id && edit.id === null ? editor :
                <button type="button" disabled={busy || edit !== null} onClick={() => start(item.id)}>＋ 新增子事项</button>}
            </>}
          </div>
        </article>
      })}
    </div>
  </section>
}
