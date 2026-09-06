import { useEffect, useMemo, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { DayState } from '../domain/model'

interface ObsidianSettings {
  vaultPath: string | null
  dailyRoot: string
}

interface DailyNotePreview {
  workDate: string
  relativePath: string
  markdown: string
  configured: boolean
}

interface SyncResult {
  workDate: string
  relativePath: string
  backupPath: string | null
  contentHash: string
}

interface DailyNoteSyncStatus {
  workDate: string
  relativePath: string
  syncState: 'clean' | 'dirty' | 'writing' | 'conflict' | 'error'
  lastError: string | null
  lastAttemptAt: string | null
  lastSuccessAt: string | null
}

const isDesktop = '__TAURI_INTERNALS__' in window
const localDate = (date = new Date()) => [date.getFullYear(), String(date.getMonth() + 1).padStart(2, '0'), String(date.getDate()).padStart(2, '0')].join('-')
const today = () => localDate()
const moveDate = (value: string, offset: number) => {
  const [year, month, day] = value.split('-').map(Number)
  return localDate(new Date(year, month - 1, day + offset, 12))
}
const clock = (value: string) => new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false }).format(new Date(value))
const statusLabels: Record<DailyNoteSyncStatus['syncState'], string> = {
  clean: '已同步', dirty: '待同步', writing: '写入中', conflict: '内容冲突', error: '写入失败',
}

export default function ObsidianSync() {
  const [openPanel, setOpenPanel] = useState(false)
  const [settings, setSettings] = useState<ObsidianSettings | null>(null)
  const [selectedDate, setSelectedDate] = useState(today)
  const [day, setDay] = useState<DayState | null>(null)
  const [preview, setPreview] = useState<DailyNotePreview | null>(null)
  const [syncStatus, setSyncStatus] = useState<DailyNoteSyncStatus | null>(null)
  const [message, setMessage] = useState('')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const requestId = useRef(0)

  const visibleEvents = useMemo(() => day?.timeline.filter((event) => event.visibility !== 'hidden') ?? [], [day])
  const completed = day?.tasks.filter((task) => task.status === 'completed').length ?? 0

  async function refreshSettings() {
    if (!isDesktop) return
    setSettings(await invoke<ObsidianSettings>('get_obsidian_settings'))
  }

  async function loadDate(workDate: string, preserveMessage = false) {
    if (!isDesktop) return
    const id = ++requestId.current
    setBusy(true)
    setError('')
    if (!preserveMessage) setMessage('')
    setDay(null); setPreview(null); setSyncStatus(null)
    try {
      const [dayResult, previewResult, statusResult] = await Promise.all([
        invoke<DayState>('get_day_snapshot', { workDate }),
        invoke<DailyNotePreview>('preview_daily_note', { workDate }),
        invoke<DailyNoteSyncStatus>('get_daily_note_sync_status', { workDate }),
      ])
      if (id !== requestId.current) return
      setDay(dayResult)
      setPreview(previewResult)
      setSyncStatus(statusResult)
    } catch (reason) {
      if (id === requestId.current) setError(String(reason))
    } finally {
      if (id === requestId.current) setBusy(false)
    }
  }

  useEffect(() => {
    const handleOpen = () => {
      const date = today()
      setSelectedDate(date)
      setOpenPanel(true)
      setError('')
      setMessage('')
      void Promise.all([refreshSettings(), loadDate(date)]).catch((reason) => setError(String(reason)))
    }
    window.addEventListener('worklog:open-obsidian', handleOpen)
    return () => window.removeEventListener('worklog:open-obsidian', handleOpen)
  }, [])

  function chooseDate(value: string) {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(value) || value > today()) return
    setSelectedDate(value)
    void loadDate(value)
  }

  async function syncSelectedDate() {
    setBusy(true)
    setError('')
    setMessage('')
    try {
      const result = await invoke<SyncResult>('sync_daily_note', { workDate: selectedDate })
      setMessage(`${selectedDate} 日记已保存：${result.relativePath}${result.backupPath ? '（原文件已备份）' : ''}`)
      await loadDate(selectedDate, true)
    } catch (reason) {
      setError(String(reason))
      try {
        setSyncStatus(await invoke<DailyNoteSyncStatus>('get_daily_note_sync_status', { workDate: selectedDate }))
      } catch {
        // 保留原始写入错误。
      }
    } finally {
      setBusy(false)
    }
  }

  function openSettings() {
    setOpenPanel(false)
    window.dispatchEvent(new Event('worklog:open-settings'))
  }

  return <>
    {openPanel && <div className="obsidian-backdrop" role="presentation" onMouseDown={() => setOpenPanel(false)}>
      <section className="obsidian-panel history-panel" role="dialog" aria-modal="true" aria-label="历史记录与 Obsidian 同步" onMouseDown={(event) => event.stopPropagation()}>
        <header>
          <div><small>DAILY ARCHIVE</small><h2>历史记录与日记</h2></div>
          <button onClick={() => setOpenPanel(false)} aria-label="关闭">×</button>
        </header>

        {!isDesktop ? <div className="obsidian-notice">请在桌面版查看历史日记。</div> : <>
          <div className="history-date-bar">
            <button type="button" disabled={busy} onClick={() => chooseDate(moveDate(selectedDate, -1))}>← 前一天</button>
            <label><span>查看日期</span><input type="date" disabled={busy} max={today()} value={selectedDate} onChange={(event) => chooseDate(event.target.value)}/></label>
            <button type="button" disabled={busy || selectedDate >= today()} onClick={() => chooseDate(moveDate(selectedDate, 1))}>后一天 →</button>
          </div>

          <div className="vault-card">
            <div><span>当前工作区</span><strong>{settings?.vaultPath ?? '尚未选择'}</strong><small>日记根目录：{settings?.dailyRoot || '工作区根目录'} · {preview?.relativePath}</small></div>
            <div className="vault-actions"><button disabled={busy} onClick={openSettings}>在设置中修改</button></div>
          </div>

          <div className="history-summary">
            <div><span>任务完成</span><strong>{completed}<small> / {day?.tasks.length ?? 0}</small></strong></div>
            <div><span>可见记录</span><strong>{visibleEvents.length}</strong></div>
            <div><span>日记状态</span><strong className={`sync-${syncStatus?.syncState ?? 'dirty'}`}>{syncStatus ? statusLabels[syncStatus.syncState] : '读取中'}</strong></div>
          </div>

          {syncStatus?.lastError && <div className="obsidian-error"><b>上次写入没有完成</b><span>{syncStatus.lastError}</span></div>}
          {message && <p className="obsidian-success">{message}</p>}
          {error && <p className="obsidian-error">{error}</p>}

          <div className="history-grid">
            <section>
              <h3>当日事项</h3>
              <div className="history-list">{!day?.tasks.length && <p>这一天没有计划事项。</p>}{day?.tasks.map((task) => <div key={task.id} className={task.status === 'completed' ? 'done' : ''}><span>{task.status === 'completed' ? '✓' : '○'}</span><b>{task.displayCode} {task.title}</b></div>)}</div>
            </section>
            <section>
              <h3>当日记录</h3>
              <div className="history-list">{!visibleEvents.length && <p>这一天没有可见记录。</p>}{visibleEvents.slice().reverse().map((event) => <div key={event.id}><time>{clock(event.occurredAt)}</time><b>{event.title}</b></div>)}</div>
            </section>
          </div>

          <div className="obsidian-actions">
            <button disabled={busy} onClick={() => void loadDate(selectedDate)}>{busy ? '读取中…' : '重新生成预览'}</button>
            <button className="sync-primary" disabled={busy || !settings?.vaultPath || !day || !preview} onClick={() => void syncSelectedDate()}>{busy ? '处理中…' : syncStatus?.syncState === 'error' ? '重试保存该日日记' : '保存该日日记'}</button>
          </div>
          {preview && <details className="markdown-preview"><summary><span>查看生成的 Markdown</span><code>{preview.relativePath}</code></summary><pre>{preview.markdown}</pre></details>}
          <footer>保存前自动备份；日记中手动添加的内容不会被覆盖。</footer>
        </>}
      </section>
    </div>}
  </>
}
