import { useEffect, useState, type FormEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { localDate } from '../domain/model'

interface ApiSession { url: string; readToken: string; writeToken: string | null }
interface AiPreview { context: unknown; question: string }

export default function ScheduleIntegrations() {
  const desktop = '__TAURI_INTERNALS__' in window
  const [open, setOpen] = useState(false)
  const [session, setSession] = useState<ApiSession | null>(null)
  const [writeAccess, setWriteAccess] = useState(false)
  const [apiBusy, setApiBusy] = useState(false)
  const [apiError, setApiError] = useState('')
  const [workDate, setWorkDate] = useState(localDate)
  const [key, setKey] = useState('')
  const [question, setQuestion] = useState('')
  const [preview, setPreview] = useState<AiPreview | null>(null)
  const [advice, setAdvice] = useState('')
  const [aiBusy, setAiBusy] = useState(false)
  const [aiError, setAiError] = useState('')

  useEffect(() => {
    const show = () => {
      setOpen(true)
      if (desktop) {
        setApiBusy(true)
        setApiError('')
        void invoke<ApiSession | null>('local_api_status').then(setSession)
          .catch((error) => setApiError(String(error))).finally(() => setApiBusy(false))
      }
    }
    window.addEventListener('worklog:open-integrations', show)
    return () => window.removeEventListener('worklog:open-integrations', show)
  }, [desktop])

  async function changeApi() {
    setApiBusy(true)
    setApiError('')
    try {
      if (session) {
        await invoke('stop_local_api')
        setSession(null)
        setWriteAccess(false)
      } else {
        setSession(await invoke<ApiSession>('start_local_api', { allowWrite: writeAccess }))
      }
    } catch (error) {
      setApiError(String(error))
    } finally {
      setApiBusy(false)
    }
  }

  async function prepareAdvice(event: FormEvent) {
    event.preventDefault()
    setPreview(null)
    setAdvice('')
    setAiError('')
    setAiBusy(true)
    try {
      const context = await invoke('preview_ai_schedule', { workDate })
      setPreview({ context, question: question.trim() })
    } catch (error) {
      setAiError(String(error))
    } finally {
      setAiBusy(false)
    }
  }

  async function sendAdvice() {
    if (!preview || !key.trim()) return
    setAiBusy(true)
    setAiError('')
    try {
      setAdvice(await invoke<string>('ask_schedule_ai', { input: { apiKey: key, ...preview } }))
      setPreview(null)
    } catch (error) {
      setAiError(String(error))
    } finally {
      setAiBusy(false)
    }
  }

  function close() {
    setOpen(false)
    setKey('')
    setQuestion('')
    setPreview(null)
    setAdvice('')
    setAiError('')
  }

  if (!open) return null
  return <div className="weekly-backdrop">
    <section className="weekly-workspace integration-workspace" role="dialog" aria-modal="true" aria-labelledby="integrations-title">
      <header><h2 id="integrations-title">日程集成</h2><button type="button" disabled={aiBusy} onClick={close}>关闭</button></header>
      {!desktop ? <p>请在桌面版使用日程集成。</p> : <>
        <section className="integration-section">
          <h3>本机日程 API</h3>
          <p>供同一台电脑上的外部程序使用。启动后发放本次会话的令牌。</p>
          {!session && <label className="integration-option"><input type="checkbox" checked={writeAccess} disabled={apiBusy} onChange={(event) => setWriteAccess(event.target.checked)}/> 同时允许修改日程</label>}
          <button type="button" className="primary small" disabled={apiBusy} onClick={() => void changeApi()}>{session ? '停止 API' : '启动 API'}</button>
          {session && <div className="integration-fields">
            <label>接口地址<input readOnly value={session.url}/></label>
            <label>只读令牌<input readOnly type="password" value={session.readToken}/></label>
            {session.writeToken && <label>写入令牌<input readOnly type="password" value={session.writeToken}/></label>}
            <div className="integration-actions">
              <button type="button" onClick={() => void navigator.clipboard.writeText(session.readToken).catch(() => setApiError('无法复制令牌'))}>复制只读令牌</button>
              {session.writeToken && <button type="button" onClick={() => void navigator.clipboard.writeText(session.writeToken!).catch(() => setApiError('无法复制令牌'))}>复制写入令牌</button>}
            </div>
          </div>}
          {apiError && <p className="weekly-error" role="alert">{apiError}</p>}
        </section>
        <section className="integration-section">
          <h3>DeepSeek 日程建议</h3>
          <form className="integration-fields" onSubmit={prepareAdvice}>
            <label>日期<input type="date" required disabled={aiBusy} value={workDate} onChange={(event) => { setWorkDate(event.target.value); setPreview(null) }}/></label>
            <label>问题<textarea required maxLength={2000} disabled={aiBusy} value={question} onChange={(event) => { setQuestion(event.target.value); setPreview(null) }} placeholder="例如：下午只有两小时，怎么安排这些任务？"/></label>
            <button type="submit" disabled={aiBusy || !question.trim()}>预览发送内容</button>
          </form>
          {preview && <div className="integration-fields">
            <p>下列问题和所选日程将发送到 DeepSeek。密钥只用于本次会话。</p>
            <pre>{preview.question}{'\n\n'}{JSON.stringify(preview.context, null, 2)}</pre>
            <label>DeepSeek API Key<input type="password" autoComplete="off" disabled={aiBusy} value={key} onChange={(event) => setKey(event.target.value)}/></label>
            <div className="integration-actions">
              <button type="button" className="primary small" disabled={aiBusy || !key.trim()} onClick={() => void sendAdvice()}>发送并获取建议</button>
              <button type="button" disabled={aiBusy} onClick={() => setPreview(null)}>取消</button>
            </div>
          </div>}
          {aiBusy && <p role="status">处理中…</p>}
          {aiError && <p className="weekly-error" role="alert">{aiError}</p>}
          {advice && <div className="integration-advice" aria-live="polite">{advice}</div>}
        </section>
      </>}
    </section>
  </div>
}
