import { useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { localDate } from '../domain/model'

interface VaultNoteSummary {
  relativePath: string
  title: string
  modifiedAt: string
  sizeBytes: number
}

interface VaultNote extends VaultNoteSummary {
  content: string
}

interface SaveResult {
  note: VaultNote
  created: boolean
  backupPath: string | null
}

const desktop = '__TAURI_INTERNALS__' in window
const defaultDirectory = () => {
  const date = localDate()
  return `随笔/${date.slice(0, 4)}/${date.slice(0, 7)}`
}
const renderObsidianLinks = (markdown: string) =>
  markdown.replace(/\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g, (_, target: string, label?: string) =>
    `[${label ?? target}](worklog-note:${encodeURIComponent(target)})`)

export default function NotesWorkspace() {
  const [visible, setVisible] = useState(false)
  const [notes, setNotes] = useState<VaultNoteSummary[]>([])
  const [note, setNote] = useState<VaultNote | null>(null)
  const [content, setContent] = useState('')
  const [savedContent, setSavedContent] = useState('')
  const [title, setTitle] = useState('')
  const [directory, setDirectory] = useState(defaultDirectory)
  const [query, setQuery] = useState('')
  const [mode, setMode] = useState<'edit' | 'split' | 'preview'>('split')
  const [newNote, setNewNote] = useState(false)
  const [external, setExternal] = useState<VaultNote | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')

  const filtered = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase()
    return needle ? notes.filter((item) => `${item.title} ${item.relativePath}`.toLocaleLowerCase().includes(needle)) : notes
  }, [notes, query])
  const dirty = content !== savedContent

  async function refreshList() {
    if (!desktop) return
    setNotes(await invoke<VaultNoteSummary[]>('list_vault_notes'))
  }

  async function openWorkspace() {
    setVisible(true)
    setError('')
    setBusy(true)
    try {
      await refreshList()
    } catch (reason) {
      setError(String(reason))
    } finally {
      setBusy(false)
    }
  }

  async function selectNote(relativePath: string) {
    if (dirty && !window.confirm('当前笔记尚未保存，确定放弃修改吗？')) return
    setBusy(true)
    setError('')
    try {
      const loaded = await invoke<VaultNote>('read_vault_note', { relativePath })
      setNote(loaded)
      setTitle(loaded.title)
      setContent(loaded.content)
      setSavedContent(loaded.content)
      setNewNote(false)
      setExternal(null)
    } catch (reason) {
      setError(String(reason))
    } finally {
      setBusy(false)
    }
  }

  function beginNote() {
    if (dirty && !window.confirm('当前笔记尚未保存，确定放弃修改吗？')) return
    setNote(null)
    setNewNote(true)
    setTitle('')
    setDirectory(defaultDirectory())
    setContent('')
    setSavedContent('')
    setExternal(null)
    setMode('split')
  }

  async function save() {
    if (!title.trim()) {
      setError('请先填写笔记标题')
      return
    }
    setBusy(true)
    setError('')
    setMessage('')
    try {
      const result = await invoke<SaveResult>('save_vault_note', {
        input: {
          workDate: localDate(),
          relativePath: newNote ? null : note?.relativePath ?? null,
          directory: newNote ? directory : null,
          title,
          content,
        },
      })
      setNote(result.note)
      setTitle(result.note.title)
      setSavedContent(result.note.content)
      setNewNote(false)
      setMessage(result.created ? '笔记已创建，并已写入今日记录。' : `笔记已保存${result.backupPath ? '，原版本已备份。' : '。'}`)
      await refreshList()
      if (result.created) window.dispatchEvent(new Event('worklog:reload'))
    } catch (reason) {
      setError(String(reason))
    } finally {
      setBusy(false)
    }
  }

  useEffect(() => {
    if (!visible || !note || newNote || !desktop) return
    const timer = window.setInterval(() => {
      void invoke<VaultNote>('read_vault_note', { relativePath: note.relativePath }).then((latest) => {
        if (latest.modifiedAt === note.modifiedAt) return
        if (content === savedContent) {
          setNote(latest)
          setTitle(latest.title)
          setContent(latest.content)
          setSavedContent(latest.content)
          setMessage('已载入 Obsidian 中的外部修改。')
        } else {
          setExternal(latest)
        }
      }).catch(() => undefined)
    }, 2000)
    return () => window.clearInterval(timer)
  }, [visible, note, newNote, content, savedContent])

  function openWikiLink(href: string) {
    const target = decodeURIComponent(href.slice('worklog-note:'.length)).toLocaleLowerCase()
    const match = notes.find((item) => item.title.toLocaleLowerCase() === target || item.relativePath.toLocaleLowerCase().replace(/\.md$/, '') === target)
    if (match) void selectNote(match.relativePath)
    else setError(`没有找到链接笔记：${decodeURIComponent(href.slice('worklog-note:'.length))}`)
  }

  const preview = <ReactMarkdown
    remarkPlugins={[remarkGfm]}
    components={{
      a: ({ href = '', children }) => href.startsWith('worklog-note:')
        ? <a href="#" onClick={(event) => { event.preventDefault(); openWikiLink(href) }}>{children}</a>
        : <a href={href} target="_blank" rel="noreferrer">{children}</a>,
    }}
  >{renderObsidianLinks(content)}</ReactMarkdown>

  return <>
    <button className="notes-fab" onClick={() => void openWorkspace()}>笔记</button>
    {visible && <div className="notes-backdrop" onMouseDown={() => setVisible(false)}>
      <section className="notes-workspace" role="dialog" aria-modal="true" aria-label="Markdown 笔记" onMouseDown={(event) => event.stopPropagation()}>
        <aside className="notes-sidebar">
          <header><div><small>OBSIDIAN VAULT</small><h2>笔记</h2></div><button onClick={beginNote}>＋ 新建</button></header>
          <input className="notes-search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索标题或路径…"/>
          {!desktop && <p className="notes-empty">笔记工作区仅在 Windows 桌面版可用。</p>}
          {desktop && filtered.length === 0 && !busy && <p className="notes-empty">工作区内还没有 Markdown 文件。</p>}
          <div className="notes-list">{filtered.map((item) => <button key={item.relativePath} className={note?.relativePath === item.relativePath ? 'active' : ''} onClick={() => void selectNote(item.relativePath)}>
            <strong>{item.title}</strong><span>{item.relativePath}</span>
          </button>)}</div>
        </aside>

        <main className="note-editor">
          <header className="note-toolbar">
            <div className="note-title-area">
              <input value={title} onChange={(event) => setTitle(event.target.value)} disabled={!newNote && Boolean(note)} placeholder="笔记标题"/>
              {newNote && <input className="note-directory" value={directory} onChange={(event) => setDirectory(event.target.value)} placeholder="保存文件夹"/>}
              {note && !newNote && <small>{note.relativePath}</small>}
            </div>
            <div className="editor-modes">
              {(['edit', 'split', 'preview'] as const).map((item) => <button key={item} className={mode === item ? 'active' : ''} onClick={() => setMode(item)}>{item === 'edit' ? '编辑' : item === 'split' ? '分栏' : '预览'}</button>)}
            </div>
            <button className="save-note" disabled={busy || (!note && !newNote) || !dirty} onClick={() => void save()}>{busy ? '保存中…' : '保存 Ctrl+S'}</button>
            <button className="close-notes" onClick={() => setVisible(false)}>×</button>
          </header>

          {error && <div className="note-error">{error}</div>}
          {message && <div className="note-message">{message}</div>}
          {external && <div className="external-change"><span>Obsidian 中的文件已发生变化，且当前编辑器有未保存内容。</span><button onClick={() => { setNote(external); setContent(external.content); setSavedContent(external.content); setExternal(null) }}>载入外部版本</button><button onClick={() => setExternal(null)}>保留当前内容</button></div>}

          {!note && !newNote ? <div className="note-welcome"><b>选择一篇 Markdown，或新建随笔</b><span>文件直接保存在 Obsidian 工作区中，Worklog 不复制第二份正文。</span></div> : <div className={`editor-grid ${mode}`}>
            {mode !== 'preview' && <textarea className="markdown-input" value={content} onChange={(event) => setContent(event.target.value)} onKeyDown={(event) => { if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') { event.preventDefault(); void save() } }} placeholder="# 从这里开始写…"/>}
            {mode !== 'edit' && <article className="markdown-rendered">{preview}</article>}
          </div>}
        </main>
      </section>
    </div>}
  </>
}
