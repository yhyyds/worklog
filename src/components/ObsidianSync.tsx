import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'

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

const isDesktop = '__TAURI_INTERNALS__' in window
const today = () => new Intl.DateTimeFormat('en-CA').format(new Date())

export default function ObsidianSync() {
  const [openPanel, setOpenPanel] = useState(false)
  const [settings, setSettings] = useState<ObsidianSettings | null>(null)
  const [preview, setPreview] = useState<DailyNotePreview | null>(null)
  const [message, setMessage] = useState('')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    if (!isDesktop) return
    void invoke<ObsidianSettings>('get_obsidian_settings')
      .then(setSettings)
      .catch((reason) => setError(String(reason)))
  }, [])

  async function chooseVault() {
    setError('')
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selected = await open({ directory: true, multiple: false, title: '选择 Obsidian 工作区' })
    if (!selected || Array.isArray(selected)) return
    setBusy(true)
    try {
      const saved = await invoke<ObsidianSettings>('save_obsidian_settings', { vaultPath: selected })
      setSettings(saved)
      setPreview(null)
      setMessage('工作区已保存')
    } catch (reason) {
      setError(String(reason))
    } finally {
      setBusy(false)
    }
  }

  async function loadPreview() {
    setBusy(true)
    setError('')
    setMessage('')
    try {
      setPreview(await invoke<DailyNotePreview>('preview_daily_note', { workDate: today() }))
    } catch (reason) {
      setError(String(reason))
    } finally {
      setBusy(false)
    }
  }

  async function syncToday() {
    setBusy(true)
    setError('')
    setMessage('')
    try {
      const result = await invoke<SyncResult>('sync_daily_note', { workDate: today() })
      setMessage(`同步完成：${result.relativePath}${result.backupPath ? '（原文件已备份）' : ''}`)
      await loadPreview()
    } catch (reason) {
      setError(String(reason))
      setBusy(false)
    }
  }

  return <>
    <button className="obsidian-fab" onClick={() => setOpenPanel(true)} title="Obsidian 同步">⬡</button>
    {openPanel && <div className="obsidian-backdrop" role="presentation" onMouseDown={() => setOpenPanel(false)}>
      <section className="obsidian-panel" role="dialog" aria-modal="true" aria-label="Obsidian 同步" onMouseDown={(event) => event.stopPropagation()}>
        <header>
          <div><small>本地 Markdown</small><h2>Obsidian 同步</h2></div>
          <button onClick={() => setOpenPanel(false)} aria-label="关闭">×</button>
        </header>

        {!isDesktop ? <div className="obsidian-notice">Obsidian 文件同步仅在 Windows 桌面版可用。浏览器开发模式不会访问本地文件。</div> : <>
          <div className="vault-card">
            <div><span>当前工作区</span><strong>{settings?.vaultPath ?? '尚未选择'}</strong><small>日记目录：{settings?.dailyRoot ?? '工作日志'}/YYYY/YYYY-MM/</small></div>
            <button disabled={busy} onClick={() => void chooseVault()}>选择工作区</button>
          </div>
          <div className="obsidian-actions">
            <button disabled={busy} onClick={() => void loadPreview()}>预览今日 Markdown</button>
            <button className="sync-primary" disabled={busy || !settings?.vaultPath} onClick={() => void syncToday()}>{busy ? '处理中…' : '同步今日日记'}</button>
          </div>
          {message && <p className="obsidian-success">{message}</p>}
          {error && <p className="obsidian-error">{error}</p>}
          {preview && <div className="markdown-preview">
            <div><span>写入目标</span><code>{preview.relativePath}</code></div>
            <pre>{preview.markdown}</pre>
          </div>}
          <footer>软件只更新 <code>worklog:managed</code> 区块；区块外的 Obsidian 内容会完整保留。覆盖前会自动备份。</footer>
        </>}
      </section>
    </div>}
  </>
}
