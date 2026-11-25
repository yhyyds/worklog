import { useEffect, useState, type FormEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { localDate, type Importance, type Urgency } from '../domain/model'
import { WORKLOAD_LABELS, type Workload } from '../domain/longTerm'

interface ApiSession { url: string; readToken: string; writeToken: string | null; mcpMode: boolean }
interface AiSnapshot { context: unknown; revision: string }
interface AiPreview extends AiSnapshot { question: string; workDate: string }
type ApiMode = 'mcp' | 'read' | 'write'
type AiPurpose = 'create' | 'advice'
interface TaskTime {
  title: string
  plannedStart: string | null
  plannedEnd: string | null
}
interface TaskGroup extends TaskTime {
  importance: Importance
  urgency: Urgency
  subtasks: Array<TaskTime & { workload: Workload }>
}
type Proposal = {
  id: string
  workDate: string
} & ({
  kind: 'reschedule'
  displayCode: string
  title: string
  oldStart: string | null
  oldEnd: string | null
  plannedStart: string
  plannedEnd: string
} | { kind: 'createTaskGroup'; plan: TaskGroup })

function timeframe(start: string | null, end: string | null) {
  return start && end ? `${start}–${end}` : '未安排时间'
}

export default function ScheduleIntegrations() {
  const desktop = '__TAURI_INTERNALS__' in window
  const [open, setOpen] = useState(false)
  const [session, setSession] = useState<ApiSession | null>(null)
  const [apiMode, setApiMode] = useState<ApiMode>('mcp')
  const [apiBusy, setApiBusy] = useState(false)
  const [apiError, setApiError] = useState('')
  const [proposals, setProposals] = useState<Proposal[]>([])
  const [proposalBusy, setProposalBusy] = useState(false)
  const [proposalError, setProposalError] = useState('')
  const [proposalStatus, setProposalStatus] = useState('')
  const [workDate, setWorkDate] = useState(localDate)
  const [purpose, setPurpose] = useState<AiPurpose>('create')
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

  useEffect(() => {
    if (!open || !desktop || proposalBusy || aiBusy) return
    let disposed = false
    let timer: ReturnType<typeof setTimeout>
    async function refresh() {
      try {
        const pending = await invoke<Proposal[]>('list_schedule_proposals')
        if (!disposed) setProposals(pending)
      } catch (error) {
        if (!disposed) setProposalError(String(error))
      } finally {
        if (!disposed) timer = setTimeout(() => void refresh(), 1500)
      }
    }
    void refresh()
    return () => { disposed = true; clearTimeout(timer) }
  }, [open, desktop, session, proposalBusy, aiBusy])

  async function changeApi() {
    setApiBusy(true)
    setApiError('')
    try {
      if (session) {
        await invoke('stop_local_api')
        setSession(null)
      } else {
        setSession(await invoke<ApiSession>('start_local_api', { allowWrite: apiMode === 'write', mcpMode: apiMode === 'mcp' }))
      }
    } catch (error) {
      setApiError(String(error))
    } finally {
      setApiBusy(false)
    }
  }

  async function decideProposal(id: string, approve: boolean) {
    setProposalBusy(true)
    setProposalError('')
    setProposalStatus('')
    try {
      await invoke('decide_schedule_proposal', { id, approve })
      setProposals((pending) => pending.filter((item) => item.id !== id))
      setProposalStatus(approve ? '已保存提案。' : '已拒绝提案。')
    } catch (error) {
      setProposalError(String(error))
    } finally {
      setProposalBusy(false)
    }
  }

  async function prepareAdvice(event: FormEvent) {
    event.preventDefault()
    setPreview(null)
    setAdvice('')
    setAiError('')
    setAiBusy(true)
    try {
      const snapshot = await invoke<AiSnapshot>('preview_ai_schedule', { workDate })
      setPreview({ ...snapshot, workDate, question: question.trim() })
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
      const input = { apiKey: key, question: preview.question }
      const result = purpose === 'create'
        ? await invoke<string>('propose_ai_task', { input: { ...input, workDate: preview.workDate, expectedRevision: preview.revision } })
        : await invoke<string>('ask_schedule_ai', { input: { ...input, context: preview.context } })
      setAdvice(result)
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
    setProposalStatus('')
    setProposalError('')
  }

  if (!open) return null
  return <div className="weekly-backdrop">
    <section className="weekly-workspace integration-workspace" role="dialog" aria-modal="true" aria-labelledby="integrations-title">
      <header><h2 id="integrations-title">日程集成</h2><button type="button" disabled={aiBusy || proposalBusy} onClick={close}>关闭</button></header>
      {!desktop ? <p>请在桌面版使用日程集成。</p> : <>
        <section className="integration-section">
          <h3>DeepSeek 日程助手</h3>
          <form className="integration-fields" onSubmit={prepareAdvice}>
            <label>用途<select value={purpose} disabled={aiBusy} onChange={(event) => { setPurpose(event.target.value as AiPurpose); setPreview(null); setAdvice(''); setAiError('') }}>
              <option value="create">起草新任务</option>
              <option value="advice">获取排期建议</option>
            </select></label>
            <label>日期<input type="date" required min={purpose === 'create' ? localDate() : undefined} disabled={aiBusy} value={workDate} onChange={(event) => { setWorkDate(event.target.value); setPreview(null) }}/></label>
            <label>{purpose === 'create' ? '任务需求' : '问题'}<textarea required maxLength={2000} disabled={aiBusy} value={question} onChange={(event) => { setQuestion(event.target.value); setPreview(null) }} placeholder={purpose === 'create'
              ? '例如：下午准备客户汇报，放在重要·紧急，拆为数据核对和制作材料，给出工作量和可用时段。'
              : '例如：下午只有两小时，怎么安排这些任务？'}/></label>
            <button type="submit" disabled={aiBusy || !question.trim()}>预览发送内容</button>
          </form>
          {preview && <div className="integration-fields">
            <p>下列问题和所选日程将发送到 DeepSeek。密钥只用于本次会话。</p>
            <pre>{preview.question}{'\n\n'}{JSON.stringify(preview.context, null, 2)}</pre>
            <label>DeepSeek API Key<input type="password" autoComplete="off" disabled={aiBusy} value={key} onChange={(event) => setKey(event.target.value)}/></label>
            <div className="integration-actions">
              <button type="button" className="primary small" disabled={aiBusy || !key.trim()} onClick={() => void sendAdvice()}>{purpose === 'create' ? '发送并起草任务' : '发送并获取建议'}</button>
              <button type="button" disabled={aiBusy} onClick={() => setPreview(null)}>取消</button>
            </div>
          </div>}
          {aiBusy && <p role="status">处理中…</p>}
          {aiError && <p className="weekly-error" role="alert">{aiError}</p>}
          {advice && <div className="integration-advice" aria-live="polite">{advice}</div>}
        </section>
        <section className="integration-section">
          <h3>待确认的日程提案</h3>
          {proposals.length === 0 && <p>当前没有待确认的提案。</p>}
          {proposals.length > 0 && <ul className="integration-proposals">
            {proposals.map((item) => <li key={item.id}>
              {item.kind === 'reschedule' ? <>
                <strong>{item.displayCode} {item.title}</strong>
                <p>{item.workDate} · {timeframe(item.oldStart, item.oldEnd)} → {timeframe(item.plannedStart, item.plannedEnd)}</p>
              </> : <>
                <strong>{item.plan.title}</strong>
                <p>{item.workDate} · {item.plan.importance === 'important' ? '重要' : '次要'} · {item.plan.urgency === 'urgent' ? '紧急' : '稍缓'} · {timeframe(item.plan.plannedStart, item.plan.plannedEnd)}</p>
                {item.plan.subtasks.length > 0 && <ul className="integration-subtasks">
                  {item.plan.subtasks.map((task, index) => <li key={index}>{task.title} · {timeframe(task.plannedStart, task.plannedEnd)} · {WORKLOAD_LABELS[task.workload]}</li>)}
                </ul>}
              </>}
              <div className="integration-actions">
                <button type="button" className="primary small" disabled={proposalBusy || apiBusy} onClick={() => void decideProposal(item.id, true)}>{item.kind === 'reschedule' ? '批准改期' : '批准创建'}</button>
                <button type="button" disabled={proposalBusy || apiBusy} onClick={() => void decideProposal(item.id, false)}>拒绝</button>
              </div>
            </li>)}
          </ul>}
          {proposalStatus && <p role="status">{proposalStatus}</p>}
          {proposalError && <p className="weekly-error" role="alert">{proposalError}</p>}
        </section>
        <details className="integration-section">
          <summary>外部软件接口 {session ? '· 运行中' : ''}</summary>
          <p>供同一台电脑上的外部程序使用。启动后发放本次会话的令牌。</p>
          {!session && <label className="integration-option">接口模式 <select value={apiMode} disabled={apiBusy} onChange={(event) => setApiMode(event.target.value as ApiMode)}>
            <option value="mcp">MCP · 提案需确认</option>
            <option value="read">API · 只读</option>
            <option value="write">API · 允许直接修改日程</option>
          </select></label>}
          <button type="button" className="primary small" disabled={apiBusy || proposalBusy} onClick={() => void changeApi()}>{session ? '停止接口' : '启动接口'}</button>
          {session && <div className="integration-fields">
            <label>接口地址<input readOnly value={session.url}/></label>
            <label>{session.mcpMode ? 'MCP 令牌' : '只读令牌'}<input readOnly type="password" value={session.readToken}/></label>
            {session.writeToken && <label>写入令牌<input readOnly type="password" value={session.writeToken}/></label>}
            <div className="integration-actions">
              <button type="button" onClick={() => void navigator.clipboard.writeText(session.readToken).catch(() => setApiError('无法复制令牌'))}>{session.mcpMode ? '复制 MCP 令牌' : '复制只读令牌'}</button>
              {session.writeToken && <button type="button" onClick={() => void navigator.clipboard.writeText(session.writeToken!).catch(() => setApiError('无法复制令牌'))}>复制写入令牌</button>}
            </div>
          </div>}
          {apiError && <p className="weekly-error" role="alert">{apiError}</p>}
        </details>
      </>}
    </section>
  </div>
}
