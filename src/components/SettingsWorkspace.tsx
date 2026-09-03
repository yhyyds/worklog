import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { TimerSettings } from '../application/gateway'

interface ObsidianSettings {
  vaultPath: string | null
  dailyRoot: string
}

interface StorageSettings {
  currentDirectory: string
  databasePath: string
  defaultDirectory: string
  isDefault: boolean
}

interface StorageMigration {
  settings: StorageSettings
  previousDatabasePath: string
}

const desktop = '__TAURI_INTERNALS__' in window
const defaultTimer: TimerSettings = {
  workMinutes: 25,
  shortBreakMinutes: 5,
  longBreakMinutes: 15,
  longBreakInterval: 4,
  autoStartBreak: true,
}

export default function SettingsWorkspace() {
  const [openPanel, setOpenPanel] = useState(false)
  const [timer, setTimer] = useState<TimerSettings>(defaultTimer)
  const [obsidian, setObsidian] = useState<ObsidianSettings | null>(null)
  const [storage, setStorage] = useState<StorageSettings | null>(null)
  const [loading, setLoading] = useState(false)
  const [savingTimer, setSavingTimer] = useState(false)
  const [timerMessage, setTimerMessage] = useState('')
  const [pathMessage, setPathMessage] = useState('')
  const [error, setError] = useState('')

  async function loadSettings() {
    if (!desktop) return
    setLoading(true)
    setError('')
    const [timerResult, obsidianResult, storageResult] = await Promise.allSettled([
      invoke<TimerSettings>('get_timer_settings'),
      invoke<ObsidianSettings>('get_obsidian_settings'),
      invoke<StorageSettings>('get_storage_settings'),
    ])
    if (timerResult.status === 'fulfilled') setTimer(timerResult.value)
    if (obsidianResult.status === 'fulfilled') setObsidian(obsidianResult.value)
    if (storageResult.status === 'fulfilled') setStorage(storageResult.value)

    const failures = [timerResult, obsidianResult, storageResult]
      .filter((result): result is PromiseRejectedResult => result.status === 'rejected')
      .map((result) => String(result.reason))
    if (failures.length) setError(failures.join('；'))
    setLoading(false)
  }

  useEffect(() => {
    const handleOpen = () => {
      setOpenPanel(true)
      setTimerMessage('')
      setPathMessage('')
      void loadSettings()
    }
    window.addEventListener('worklog:open-settings', handleOpen)
    return () => window.removeEventListener('worklog:open-settings', handleOpen)
  }, [])

  async function saveTimer() {
    setSavingTimer(true)
    setTimerMessage('')
    setError('')
    try {
      setTimer(await invoke<TimerSettings>('save_timer_settings', { settings: timer }))
      setTimerMessage('番茄钟设置已保存，下一轮专注开始生效。')
    } catch (reason) {
      setError(String(reason))
    } finally {
      setSavingTimer(false)
    }
  }

  async function chooseStorage() {
    setError('')
    setPathMessage('')
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择 Worklog 本地数据文件夹（请选择空文件夹）',
      defaultPath: storage?.currentDirectory,
    })
    if (!selected || Array.isArray(selected)) return
    setLoading(true)
    try {
      const result = await invoke<StorageMigration>('migrate_storage_directory', { directory: selected })
      setStorage(result.settings)
      setPathMessage(`数据库已迁移并立即切换。原数据库仍保留为安全备份：${result.previousDatabasePath}`)
      window.dispatchEvent(new Event('worklog:reload'))
    } catch (reason) {
      setError(String(reason))
    } finally {
      setLoading(false)
    }
  }

  async function chooseVault() {
    setError('')
    setPathMessage('')
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择 Obsidian 工作区',
      defaultPath: obsidian?.vaultPath ?? undefined,
    })
    if (!selected || Array.isArray(selected)) return
    setLoading(true)
    try {
      setObsidian(await invoke<ObsidianSettings>('save_obsidian_settings', { vaultPath: selected }))
      setPathMessage('Obsidian 工作区已保存；日记根目录已恢复为工作区根目录。')
    } catch (reason) {
      setError(String(reason))
    } finally {
      setLoading(false)
    }
  }

  async function chooseDailyRoot() {
    if (!obsidian?.vaultPath) {
      setError('请先选择 Obsidian 工作区。')
      return
    }
    setError('')
    setPathMessage('')
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择日记根目录（必须位于 Obsidian 工作区内）',
      defaultPath: obsidian.vaultPath,
    })
    if (!selected || Array.isArray(selected)) return
    setLoading(true)
    try {
      setObsidian(await invoke<ObsidianSettings>('save_daily_root', { dailyPath: selected }))
      setPathMessage('日记根目录已保存。')
    } catch (reason) {
      setError(String(reason))
    } finally {
      setLoading(false)
    }
  }

  if (!openPanel) return null
  return <div className="settings-backdrop" onMouseDown={() => setOpenPanel(false)}>
    <section className="settings-workspace" role="dialog" aria-modal="true" aria-label="设置" onMouseDown={(event) => event.stopPropagation()}>
      <header className="settings-header">
        <div><small>WORKLOG PREFERENCES</small><h2>设置</h2><p>集中管理专注节奏与本地文件位置</p></div>
        <button type="button" onClick={() => setOpenPanel(false)} aria-label="关闭设置">×</button>
      </header>

      {!desktop && <div className="settings-notice">本地路径与系统通知仅在 Windows 桌面版可用。</div>}
      {error && <div className="settings-error"><span>{error}</span><button type="button" onClick={() => setError('')}>关闭</button></div>}

      <div className="settings-content">
        <section className="settings-section">
          <div className="settings-section-title"><span>01</span><div><h3>番茄钟</h3><p>控制每一轮工作的节奏；正在进行的计时不会被中途改变。</p></div></div>
          <div className="settings-number-grid">
            <label><span>单轮专注</span><div><input type="number" min="1" max="180" value={timer.workMinutes} onChange={(event) => setTimer({ ...timer, workMinutes: Number(event.target.value) })}/><small>分钟</small></div></label>
            <label><span>短休息</span><div><input type="number" min="1" max="60" value={timer.shortBreakMinutes} onChange={(event) => setTimer({ ...timer, shortBreakMinutes: Number(event.target.value) })}/><small>分钟</small></div></label>
            <label><span>长休息</span><div><input type="number" min="1" max="120" value={timer.longBreakMinutes} onChange={(event) => setTimer({ ...timer, longBreakMinutes: Number(event.target.value) })}/><small>分钟</small></div></label>
            <label><span>长休息间隔</span><div><input type="number" min="1" max="12" value={timer.longBreakInterval} onChange={(event) => setTimer({ ...timer, longBreakInterval: Number(event.target.value) })}/><small>轮</small></div></label>
          </div>
          <label className="settings-switch"><input type="checkbox" checked={timer.autoStartBreak} onChange={(event) => setTimer({ ...timer, autoStartBreak: event.target.checked })}/><span><b>专注结束后自动开始休息</b><small>工作结束、休息开始和休息结束仍会分别发送系统提醒。</small></span></label>
          <div className="settings-section-footer"><span>{timerMessage || '全局唤起快捷键：Ctrl + Shift + W'}</span><button type="button" disabled={!desktop || loading || savingTimer} onClick={() => void saveTimer()}>{savingTimer ? '保存中…' : '保存番茄钟设置'}</button></div>
        </section>

        <section className="settings-section">
          <div className="settings-section-title"><span>02</span><div><h3>本地数据</h3><p>任务、时间线与专注记录保存在 SQLite 数据库中。</p></div></div>
          <div className="settings-path-card">
            <div><span>当前数据目录</span><strong>{storage?.currentDirectory ?? (loading ? '正在读取…' : '未读取')}</strong><small>数据库：{storage?.databasePath ?? 'worklog.db'}</small></div>
            <button type="button" disabled={!desktop || loading} onClick={() => void chooseStorage()}>更改并迁移</button>
          </div>
          <p className="settings-path-hint">请选择空文件夹。迁移会复制完整数据库、立即切换到新位置，并保留原文件作为安全备份。</p>
        </section>

        <section className="settings-section">
          <div className="settings-section-title"><span>03</span><div><h3>Obsidian 与日记</h3><p>工作区负责浏览 Markdown；日记可输出到工作区内任意指定文件夹。</p></div></div>
          <div className="settings-path-card">
            <div><span>Obsidian 工作区</span><strong>{obsidian?.vaultPath ?? '尚未选择'}</strong><small>用于随笔、浏览与日记同步</small></div>
            <button type="button" disabled={!desktop || loading} onClick={() => void chooseVault()}>选择工作区</button>
          </div>
          <div className="settings-path-card">
            <div><span>日记根目录</span><strong>{obsidian?.dailyRoot || (obsidian?.vaultPath ? '工作区根目录' : '请先选择工作区')}</strong><small>最终输出：YYYY/YYYY-MM/YYYY-MM-DD.md</small></div>
            <button type="button" disabled={!desktop || loading || !obsidian?.vaultPath} onClick={() => void chooseDailyRoot()}>选择日记目录</button>
          </div>
          {pathMessage && <p className="settings-success">{pathMessage}</p>}
        </section>
      </div>
    </section>
  </div>
}
