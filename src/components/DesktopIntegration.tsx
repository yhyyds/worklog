import { useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

const desktop = '__TAURI_INTERNALS__' in window

export default function DesktopIntegration() {
  useEffect(() => {
    if (!desktop) return
    let disposed = false
    const cleanups: Array<() => void | Promise<void>> = []
    const windowHandle = getCurrentWindow()

    void (async () => {
      try {
        const notification = await import('@tauri-apps/plugin-notification')
        if (!await notification.isPermissionGranted()) await notification.requestPermission()
      } catch (reason) {
        console.warn('Notification integration unavailable', reason)
      }

      try {
        const shortcuts = await import('@tauri-apps/plugin-global-shortcut')
        await shortcuts.register('CommandOrControl+Shift+W', (event) => {
          if (event.state === 'Pressed') {
            void windowHandle.unminimize()
            void windowHandle.show()
            void windowHandle.setFocus()
          }
        })
        cleanups.push(() => shortcuts.unregister('CommandOrControl+Shift+W'))
      } catch (reason) {
        console.warn('Global shortcut integration unavailable', reason)
      }

      try {
        const unlisten = await listen('worklog-timer-changed', () => window.dispatchEvent(new Event('worklog:reload')))
        if (disposed) unlisten()
        else cleanups.push(unlisten)
      } catch (reason) {
        console.warn('Timer event integration unavailable', reason)
      }

    })()

    return () => {
      disposed = true
      for (const cleanup of cleanups) void cleanup()
    }
  }, [])

  return null
}
