import { useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { createGateway } from '../infrastructure/createGateway'
import { localDate, type DayTask } from '../domain/model'
import type { CloseDayResult, EndOfDayPreview } from '../application/gateway'

interface DailyNotePreview {
  markdown: string
  configured: boolean
  relativePath: string
}

const statusLabel: Record<string, string> = {
  not_started: '未开始', in_progress: '进行中', waiting: '等待他人',
  blocked: '阻塞', deferred: '延期',
}

export default function EndOfDay() {
  const gateway = useMemo(createGateway, [])
  const [visible, setVisible] = useState(false)
  const [preview, setPreview] = useState<EndOfDayPreview | null>(null)
  const [markdown, setMarkdown] = useState<DailyNotePreview | null>(null)
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [result, setResult] = useState<CloseDayResult | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')

  async function openPanel() {
    setVisible(true)
    setBusy(true)
    setError('')
    setMessage('')
    setResult(null)
    try {
      const day = await gateway.previewEndOfDay(localDate())
      setPreview(day)
      setSelected(new Set(day.candidates.map((task) => task.instanceId)))
      if ('__TAURI_INTERNALS__' in window) {
        setMarkdown(await invoke<DailyNotePreview>('preview_daily_note', { workDate: day.workDate }))
      }
    } catch (reason) {
      setError(String(reason))
    } finally {
      setBusy(false)
    }
  }

  function toggle(id: string) {
    setSelected((current) => {
      const next = new Set(current)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  async function confirmClose() {
    if (!preview) return
    setBusy(true)
    setError('')
    setMessage('')
    try {
      const closed = await gateway.closeDay({
        workDate: preview.workDate,
        nextWorkDate: preview.nextWorkDate,
        selectedInstanceIds: [...selected],
      })
      setResult(closed)
      window.dispatchEvent(new Event('worklog:reload'))
      if ('__TAURI_INTERNALS__' in window && markdown?.configured) {
        try {
          await invoke('sync_daily_note', { workDate: preview.workDate })
          setMessage('日终收尾完成，今日日记已同步到 Obsidian。')
        } catch (reason) {
          setMessage(`任务顺延已完成，但 Markdown 同步失败：${String(reason)}`)
        }
      } else {
        setMessage('日终收尾完成；尚未配置 Obsidian，因此没有写入 Markdown。')
      }
    } catch (reason) {
      setError(String(reason))
    } finally {
      setBusy(false)
    }
  }

  const selectedTasks = preview?.candidates.filter((task) => selected.has(task.instanceId)) ?? []
  const selectedIds = new Set(selectedTasks.map((task) => task.instanceId))

  return <>
    <button className="closing-fab" onClick={() => void openPanel()}>日终</button>
    {visible && <div className="closing-backdrop" onMouseDown={() => setVisible(false)}>
      <section className="closing-panel" role="dialog" aria-modal="true" aria-label="日终收尾" onMouseDown={(event) => event.stopPropagation()}>
        <header>
          <div><small>END OF DAY</small><h2>日终收尾</h2><p>确认今天的成果，只把仍需继续的部分带到明天。</p></div>
          <button onClick={() => setVisible(false)} aria-label="关闭">×</button>
        </header>

        {busy && !preview && <div className="closing-loading">正在整理今天的记录…</div>}
        {error && <p className="closing-error">{error}</p>}
        {preview && <>
          <div className="closing-summary">
            <div><strong>{preview.completedCount}</strong><span>已完成</span></div>
            <div><strong>{preview.candidates.length}</strong><span>可顺延</span></div>
            <div><strong>{preview.waitingCount}</strong><span>等待他人</span></div>
            <div><strong>{preview.blockedCount}</strong><span>阻塞</span></div>
          </div>

          {preview.alreadyClosed ? <div className="closing-done">今天已经完成日终收尾，无需再次顺延。</div> : !result && <>
            <div className="carry-heading"><div><h3>顺延到 {preview.nextWorkDate}</h3><p>次日会重新编号；勾选子任务但不勾选父任务时，子任务会提升为顶级事项。</p></div><button onClick={() => setSelected(new Set(preview.candidates.map((task) => task.instanceId)))}>全选</button></div>
            <div className="carry-list">
              {preview.candidates.length === 0 && <p className="carry-empty">没有需要顺延的事项，可以直接完成收尾。</p>}
              {preview.candidates.map((task) => {
                const promoted = Boolean(task.parentId && !selectedIds.has(task.parentId) && selected.has(task.instanceId))
                return <label key={task.instanceId} className={`carry-task ${task.parentId ? 'child' : ''}`}>
                  <input type="checkbox" checked={selected.has(task.instanceId)} onChange={() => toggle(task.instanceId)}/>
                  <span><strong>{task.displayCode} {task.title}</strong><small>{statusLabel[task.status] ?? task.status}{promoted ? ' · 次日提升为顶级事项' : ''}</small></span>
                </label>
              })}
            </div>
            {markdown && <details className="closing-markdown"><summary>查看今日日记写入预览</summary><code>{markdown.relativePath}</code><pre>{markdown.markdown}</pre></details>}
            <footer>
              <span>将顺延 {selected.size} 项；已完成及已取消事项不会复制。</span>
              <button className="close-day-primary" disabled={busy} onClick={() => void confirmClose()}>{busy ? '处理中…' : '确认收尾并生成明日任务'}</button>
            </footer>
          </>}

          {result && <div className="closing-result">
            <div className="result-mark">✓</div>
            <h3>今天已妥善收好</h3>
            <p>{message}</p>
            <h4>明日任务草稿 · {result.nextDay.workDate}</h4>
            <ul>{result.nextDay.tasks.map((task: DayTask) => <li key={task.id}><span>{task.displayCode}</span>{task.title}</li>)}</ul>
          </div>}
        </>}
      </section>
    </div>}
  </>
}
