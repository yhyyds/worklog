import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { TimerSettings } from '../application/gateway'

const desktop = '__TAURI_INTERNALS__' in window

export default function DesktopIntegration() {
  const [open, setOpen] = useState(false)
  const [settings, setSettings] = useState<TimerSettings | null>(null)
  const [message, setMessage] = useState('')

  useEffect(() => {
    const handleOpen = () => setOpen(true)
    window.addEventListener('worklog:open-settings', handleOpen)
    return () => window.removeEventListener('worklog:open-settings', handleOpen)
  }, [])

  useEffect(() => {
    if (!desktop) return
    let disposed = false
    const windowHandle = getCurrentWindow()
    let unregisterShortcut: (() => Promise<void>) | undefined
    let unlistenTimer: (() => void) | undefined
    let unlistenClose: (() => void) | undefined

    void (async () => {
      const notification = await import('@tauri-apps/plugin-notification')
      if (!await notification.isPermissionGranted()) await notification.requestPermission()

      const shortcuts = await import('@tauri-apps/plugin-global-shortcut')
      await shortcuts.register('CommandOrControl+Shift+W', (event) => {
        if (event.state === 'Pressed') {
          void windowHandle.unminimize()
          void windowHandle.show()
          void windowHandle.setFocus()
        }
      })
      unregisterShortcut = () => shortcuts.unregister('CommandOrControl+Shift+W')
      unlistenTimer = await listen('worklog-timer-changed', () => window.dispatchEvent(new Event('worklog:reload')))
      unlistenClose = await windowHandle.onCloseRequested((event) => {
        event.preventDefault()
        void windowHandle.hide()
      })
      if (!disposed) setSettings(await invoke<TimerSettings>('get_timer_settings'))
    })().catch((reason) => setMessage(String(reason)))

    return () => {
      disposed = true
      unlistenTimer?.()
      unlistenClose?.()
      void unregisterShortcut?.()
    }
  }, [])

  async function save() {
    if (!settings) return
    try {
      setSettings(await invoke<TimerSettings>('save_timer_settings', { settings }))
      setMessage('番茄钟设置已保存。')
      setOpen(false)
    } catch (reason) {
      setMessage(String(reason))
    }
  }

  if (!desktop) return null
  return <>
    <button className="timer-settings-fab" onClick={() => setOpen(true)} title="番茄钟设置">计时</button>
    {open && settings && <div className="timer-settings-backdrop" onMouseDown={() => setOpen(false)}>
      <section className="timer-settings-panel" role="dialog" aria-modal="true" onMouseDown={(event) => event.stopPropagation()}>
        <header><div><small>FOCUS CYCLE</small><h2>番茄钟设置</h2></div><button onClick={() => setOpen(false)}>×</button></header>
        <div className="timer-setting-grid">
          <label><span>专注时长</span><input type="number" min="1" max="180" value={settings.workMinutes} onChange={(event) => setSettings({ ...settings, workMinutes: Number(event.target.value) })}/><small>分钟</small></label>
          <label><span>短休息</span><input type="number" min="1" max="60" value={settings.shortBreakMinutes} onChange={(event) => setSettings({ ...settings, shortBreakMinutes: Number(event.target.value) })}/><small>分钟</small></label>
          <label><span>长休息</span><input type="number" min="1" max="120" value={settings.longBreakMinutes} onChange={(event) => setSettings({ ...settings, longBreakMinutes: Number(event.target.value) })}/><small>分钟</small></label>
          <label><span>长休息间隔</span><input type="number" min="1" max="12" value={settings.longBreakInterval} onChange={(event) => setSettings({ ...settings, longBreakInterval: Number(event.target.value) })}/><small>轮</small></label>
        </div>
        <label className="auto-break"><input type="checkbox" checked={settings.autoStartBreak} onChange={(event) => setSettings({ ...settings, autoStartBreak: event.target.checked })}/><span><b>专注结束后自动开始休息</b><small>系统会分别提醒专注结束和休息开始。</small></span></label>
        <p className="shortcut-hint">全局快捷键：<kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>W</kbd> 唤起 Worklog</p>
        {message && <p className="timer-setting-message">{message}</p>}
        <footer><button onClick={() => void save()}>保存设置</button></footer>
      </section>
    </div>}
  </>
}
