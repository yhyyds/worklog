import { createContext, useContext, useEffect, useState, type ReactNode, type FormEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'

interface Category { id: string; name: string; color: string; shareMode: 'public' | 'anonymous' | 'excluded' }
interface Assignment { entityId: string; entityKind: 'habit' | 'goal'; categoryId: string | null; shareName: boolean }
interface Catalog { categories: Category[]; classifications: Assignment[] }
const Context = createContext<{ catalog: Catalog; update: (command: string, input: unknown) => Promise<void>; busy: boolean } | null>(null)
export function GrowthCategories({ children }: { children: ReactNode }) {
  const [catalog, setCatalog] = useState<Catalog>({ categories: [], classifications: [] })
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  useEffect(() => { if ('__TAURI_INTERNALS__' in window) void invoke<Catalog>('get_growth_catalog').then(setCatalog).catch(e => setError(String(e))) }, [])
  async function update(command: string, input: unknown) {
    setBusy(true); setError('')
    try { setCatalog(await invoke<Catalog>(command, { input })); window.dispatchEvent(new Event('worklog:categories-changed')) }
    catch (e) { setError(String(e)); throw e }
    finally { setBusy(false) }
  }
  return <Context.Provider value={{ catalog, update, busy }}>{error && <p className="growth-error" role="alert">{error}</p>}{children}</Context.Provider>
}
export function CategoryPicker({ entityId, kind }: { entityId: string; kind: 'habit' | 'goal' }) {
  const context = useContext(Context)!
  const { catalog, busy, update } = context
  const assignment = catalog.classifications.find(a => a.entityId === entityId)
  const selected = catalog.categories.find(c => c.id === assignment?.categoryId)
  const save = (categoryId: string | null, shareName: boolean) => { void update('assign_growth_category', { entityId, entityKind: kind, categoryId, shareName }).catch(() => undefined) }
  return <div className="category-picker">
    <span className="category-dot" style={{ background: selected?.color ?? '#87938b' }}/>
    <select aria-label="事项分类" disabled={busy} value={assignment?.categoryId ?? ''} onChange={e => save(e.target.value || null, false)}><option value="">未分类</option>{catalog.categories.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}</select>
    {selected?.shareMode === 'public' && <label><input type="checkbox" disabled={busy} checked={assignment?.shareName ?? false} onChange={e => save(selected.id, e.target.checked)}/>分享具体名称</label>}
    {selected && <small>{selected.shareMode === 'public' ? '分享分类统计' : selected.shareMode === 'anonymous' ? '仅计入分享总数' : '不参与分享'}</small>}
  </div>
}
export function CategoryManager() {
  const { catalog, update, busy } = useContext(Context)!
  return <section className="category-manager"><h3>分类与分享</h3><p>打卡与长期目标共用分类。公开分类会出现在分享周报中；事项名称需要另外勾选。</p>
    <CategoryForm busy={busy} onSave={input => update('save_growth_category', input)}/>
    {catalog.categories.map(c => <CategoryForm key={`${c.id}-${c.name}-${c.color}-${c.shareMode}`} category={c} busy={busy} onSave={input => update('save_growth_category', input)}/>)}
    <details><summary>分享方式说明</summary><p>公开：展示分类名称、颜色和完成情况。匿名汇总：只计入整体统计，不展示分类名称、颜色和逐项记录。不参与分享：连同该分类目标任务的专注时长一起排除。未分类事项默认匿名汇总。</p><p>隐私设置只影响分享预览和图片，不会修改本地记录，也不会加密数据库。</p></details>
  </section>
}
function CategoryForm({ category, busy, onSave }: { category?: Category; busy: boolean; onSave: (input: Category) => Promise<void> }) {
  const [name, setName] = useState(category?.name ?? '')
  const [color, setColor] = useState(category?.color ?? '#5b9b7d')
  const [shareMode, setShareMode] = useState<Category['shareMode']>(category?.shareMode ?? 'anonymous')
  async function submit(e: FormEvent) { e.preventDefault(); try { await onSave({ id: category?.id ?? '', name, color, shareMode }); if (!category) setName('') } catch { /* Provider displays the error. */ } }
  return <form className="category-form" onSubmit={e => void submit(e)}><input aria-label="分类颜色" type="color" value={color} onChange={e => setColor(e.target.value)}/><input aria-label="分类名称" maxLength={40} value={name} onChange={e => setName(e.target.value)} placeholder="例如：生活、工作、自我提升"/><select aria-label="分类分享方式" value={shareMode} onChange={e => setShareMode(e.target.value as Category['shareMode'])}><option value="anonymous">匿名汇总</option><option value="public">公开分类</option><option value="excluded">不参与分享</option></select><button disabled={busy || !name.trim()}>{category ? '保存修改' : '新建分类'}</button></form>
}
