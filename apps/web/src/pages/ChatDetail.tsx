import { useState, useEffect, useCallback, useRef } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { createCommand, listCommands } from '../api/commands'
import { useWebSocket } from '../hooks/useWebSocket'
import { listModels } from '../api/models'
import { useAuth } from '../contexts/AuthContext'
import { MarkdownOutput } from '../components/MarkdownOutput'
import { FileViewerModal } from '../components/FileViewerModal'
import { TEMPLATES, getTemplateById, getDefaultTemplate } from '../templates'
import type { Command } from '../types'

function folderName(path: string): string {
  const normalized = path.replace(/\/$/, '')
  if (normalized === '~/repos' || normalized.endsWith('/repos')) return '_'
  return normalized.split(/[/\\]/).pop() || path
}

function generatedTitle(thread: Command[]): string {
  const first = thread[0]
  const summaryTitle = first.summary
    ?.split('\n')
    .map((line) => line.trim())
    .find((line) => /^#{1,6}\s+/.test(line))
    ?.replace(/^#{1,6}\s+/, '')
    .trim()
  const source =
    summaryTitle ||
    first.summary ||
    (first.output && first.status === 'done' ? first.output : null) ||
    first.input
  const cleaned = (source || '')
    .replace(/[`*_>#-]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
  if (!cleaned) return 'Chat'
  if (cleaned.length <= 52) return cleaned
  return `${cleaned.slice(0, 49)}...`
}

/**
 * Strip thinking/response/console markers from executor output.
 * Handles [Thinking]/[Console]/[Response] section markers and <think>...</think> tags.
 * Returns { thinking, console, response }.
 */
function parseOutput(raw: string): { thinking: string; console: string; response: string } {
  // 1. Try bracket markers (executor stream format)
  const bracketThinking = raw.match(/\[Thinking\]\n([\s\S]*?)(?=\n\n\[(?:Console|Response)\]|$)/)
  const bracketConsole = raw.match(/\[Console\]\n([\s\S]*?)(?=\n\n\[Response\]|$)/)
  const bracketResponse = raw.match(/\[Response\]\n([\s\S]*)$/)
  if (bracketThinking || bracketConsole || bracketResponse) {
    let thinking = bracketThinking?.[1]?.trim() ?? ''
    const console = bracketConsole?.[1]?.trim() ?? ''
    let response = bracketResponse?.[1]?.trim() ?? ''
    // The response itself might also contain <think> blocks
    const inner = extractThinkTags(response)
    if (inner.thinking) {
      thinking = thinking ? `${thinking}\n\n${inner.thinking}` : inner.thinking
    }
    return { thinking, console, response: inner.response }
  }
  // 2. Try <think>...</think> tags (model output format)
  const tagged = extractThinkTags(raw)
  if (tagged.thinking) {
    return { thinking: tagged.thinking, console: '', response: tagged.response }
  }
  // 3. No markers — treat entire string as the response
  return { thinking: '', console: '', response: raw.trim() }
}

/** Extract all <think>...</think> blocks from text, return thinking + remaining response. */
function extractThinkTags(text: string): { thinking: string; response: string } {
  const thinkBlocks: string[] = []
  const cleaned = text.replace(/<think>([\s\S]*?)<\/think>/g, (_match, content: string) => {
    const trimmed = content.trim()
    if (trimmed) thinkBlocks.push(trimmed)
    return ''
  })
  // Also handle unclosed <think> at the end (still streaming)
  const unclosed = cleaned.match(/<think>([\s\S]*)$/)
  let response = cleaned
  if (unclosed) {
    const trimmed = unclosed[1].trim()
    if (trimmed) thinkBlocks.push(trimmed)
    response = cleaned.slice(0, unclosed.index)
  }
  return {
    thinking: thinkBlocks.join('\n\n'),
    response: response.trim(),
  }
}

export default function ChatDetail() {
  const { chatId } = useParams<{ chatId: string }>()
  const { token } = useAuth()
  const navigate = useNavigate()
  const [commands, setCommands] = useState<Command[]>([])
  const [models, setModels] = useState<string[]>([])
  const [translatorModel, setTranslatorModel] = useState<string>('composer-1.5')
  const [workloadModel, setWorkloadModel] = useState<string>('composer-1.5')
  const [templateId, setTemplateId] = useState<string>(getDefaultTemplate().id)
  const [input, setInput] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const formRef = useRef<HTMLFormElement | null>(null)
  const scrollRef = useRef<HTMLElement | null>(null)
  const isPinnedToBottom = useRef(true)
  const [viewingFile, setViewingFile] = useState<string | null>(null)
  const SCROLL_THRESHOLD = 60

  const byCursorChat = commands.filter((c) => c.cursor_chat_id === chatId)
  const byId = commands.filter((c) => c.id === chatId)
  const thread =
    byCursorChat.length > 0
      ? byCursorChat.sort((a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime())
      : byId.sort((a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime())
  const root = thread[0]
  const latest = thread[thread.length - 1]
  const title = root ? generatedTitle(thread) : 'Chat'
  const repoPath = root?.repo_path ?? null
  const handleFileClick = useCallback(
    (filePath: string) => setViewingFile(filePath),
    []
  )

  const refreshCommands = useCallback(async () => {
    if (!token) return
    try {
      const list = await listCommands(token)
      setCommands(list)
    } catch (_) {}
  }, [token])

  const refreshModels = useCallback(async () => {
    if (!token) return
    try {
      const list = await listModels(token)
      setModels(list)
      setTranslatorModel((prev) => (list.length > 0 && !list.includes(prev) ? list[0] : prev))
      setWorkloadModel((prev) => (list.length > 0 && !list.includes(prev) ? list[0] : prev))
    } catch (_) {}
  }, [token])

  useEffect(() => {
    if (!token) {
      navigate('/login')
      return
    }
    refreshCommands()
    refreshModels()
  }, [token, navigate, refreshCommands, refreshModels])

  const handleWsMessage = useCallback(
    (event: MessageEvent) => {
      try {
        const msg = JSON.parse(event.data as string)
        if (msg.type === 'command_update' && msg.payload) {
          const { id, status, output, summary, cursor_chat_id } = msg.payload
          setCommands((prev) => {
            const idx = prev.findIndex((c) => c.id === id)
            if (idx >= 0) {
              const existing = prev[idx]
              const newStatus = status || existing.status
              const newOutput = output !== undefined ? output : existing.output
              const newSummary = summary !== undefined ? summary : existing.summary
              const newChatId = cursor_chat_id !== undefined ? cursor_chat_id : existing.cursor_chat_id
              // Bail out if nothing actually changed — avoids unnecessary re-renders
              if (
                newStatus === existing.status &&
                newOutput === existing.output &&
                newSummary === existing.summary &&
                newChatId === existing.cursor_chat_id
              ) {
                return prev
              }
              const next = [...prev]
              next[idx] = { ...existing, status: newStatus, output: newOutput, summary: newSummary, cursor_chat_id: newChatId }
              return next
            }
            refreshCommands()
            return prev
          })
          return
        }
        if (msg.type === 'command_new') {
          refreshCommands()
          return
        }
      } catch {
        /* ignore */
      }
    },
    [refreshCommands]
  )

  const { ready: wsReady, error: wsError } = useWebSocket(token, handleWsMessage)

  // Polling fallback: poll REST every 2s when a command is running, or every 10s otherwise.
  // This ensures live output even if WebSocket is down.
  useEffect(() => {
    if (!token) return
    const hasRunning = thread.some((c) => c.status === 'running' || c.status === 'pending')
    const interval = hasRunning ? 2000 : 10000
    const id = setInterval(refreshCommands, interval)
    return () => clearInterval(id)
  }, [token, refreshCommands, thread.length, thread.map((c) => c.status).join()])

  const handleScroll = useCallback(() => {
    const el = scrollRef.current
    if (!el) return
    const { scrollTop, clientHeight, scrollHeight } = el
    isPinnedToBottom.current = scrollTop + clientHeight >= scrollHeight - SCROLL_THRESHOLD
  }, [])

  useEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const msgs = byCursorChat.length > 0 ? byCursorChat : byId
    if (msgs.length === 0) return
    if (!isPinnedToBottom.current) return
    requestAnimationFrame(() => {
      el.scrollTop = el.scrollHeight
    })
  }, [chatId, commands, byCursorChat.length, byId.length])

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!token || !input.trim() || !root) return
    setError('')
    setLoading(true)
    try {
      const tpl = getTemplateById(templateId) ?? getDefaultTemplate()
      const created = await createCommand(token, {
        input: input.trim(),
        cursor_chat_id: root.cursor_chat_id || undefined,
        repo_path: root.repo_path || undefined,
        context_mode: tpl.contextMode ?? undefined,
        translator_model: translatorModel || undefined,
        workload_model: workloadModel || undefined,
      })
      setInput('')
      setCommands((prev) => {
        if (prev.some((c) => c.id === created.id)) return prev
        return [...prev, created as Command]
      })
      await refreshCommands()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed')
    } finally {
      setLoading(false)
    }
  }

  if (!token) return null

  const modelOptions = models.length ? models : ['composer-1.5']

  const fileViewer =
    viewingFile && repoPath && token ? (
      <FileViewerModal
        filePath={viewingFile}
        repoPath={repoPath}
        token={token}
        onClose={() => setViewingFile(null)}
      />
    ) : null

  return (
    <div className="chat-app">
      {fileViewer}
      <header className="chat-header">
        <button
          type="button"
          onClick={() => navigate('/chat')}
          className="chat-back"
          aria-label="Back to chats"
        >
          ←
        </button>
        <div className="chat-header-info">
          <h1 className="chat-title">{root ? title : 'Chat'}</h1>
          <p className="chat-meta">
            {root
              ? `${folderName(root.repo_path || '~/repos')} • ${latest?.status ?? '—'}${
                  thread.length > 1 ? ` • ${thread.length} msgs` : ''
                }${!wsReady ? ' • ⚡reconnecting' : ''}`
              : 'Loading…'}
          </p>
          {wsError && <p className="text-sm text-red-600">{wsError}</p>}
        </div>
        <div className="ml-auto flex shrink-0 items-center gap-1">
          <a
            href={`/chat/${chatId}/docs`}
            onClick={(e) => {
              e.preventDefault()
              navigate(`/chat/${chatId}/docs`)
            }}
            className="rounded p-1.5 text-[var(--text-soft)] hover:bg-[var(--surface-strong)] hover:text-[var(--text)]"
            aria-label="View docs"
            title="View .md docs"
          >
            Docs
          </a>
          <button
            type="button"
            onClick={() => refreshCommands()}
            className="rounded p-1.5 text-[var(--text-soft)] hover:bg-[var(--surface-strong)] hover:text-[var(--text)]"
            aria-label="Refresh"
            title="Refresh for latest updates"
          >
            ↻
          </button>
        </div>
      </header>

      <main
        ref={scrollRef}
        className="chat-scroll"
        onScroll={handleScroll}
      >
        {thread.length === 0 && !root ? (
          <p className="chat-empty">No messages. Start a conversation below.</p>
        ) : (
          thread.map((cmd, i) => {
            const parsed = cmd.output ? parseOutput(cmd.output) : null
            const hasContent = parsed && (parsed.thinking || parsed.console || parsed.response)
            return (
              <div key={cmd.id} className="chat-message">
                {cmd.input && (
                  <p className="chat-prompt">{i > 0 ? `↳ ${cmd.input}` : cmd.input}</p>
                )}
                {cmd.input && (hasContent || cmd.summary || cmd.status === 'running' || cmd.status === 'pending') && (
                  <hr className="chat-bar" />
                )}
                {(cmd.status === 'pending' || cmd.status === 'running' || cmd.status === 'failed' || cmd.status === 'done') && (
                  <>
                    {hasContent ? (
                      <>
                        {parsed.thinking && (
                          <details className="chat-thinking mb-2" open={cmd.status === 'running'}>
                            <summary className="text-xs font-medium text-[var(--text-soft)] cursor-pointer select-none">
                              Thinking{cmd.status === 'running' ? '…' : ''}
                            </summary>
                            <div className="mt-1 text-sm text-[var(--text-muted)] opacity-75">
                              <MarkdownOutput
                                content={parsed.thinking}
                                scrollable={false}
                                repoPath={repoPath}
                                onFileClick={handleFileClick}
                              />
                            </div>
                          </details>
                        )}
                        {parsed.console && (
                          <details className="chat-console mb-2" open={cmd.status === 'running'}>
                            <summary className="text-xs font-medium text-[var(--text-soft)] cursor-pointer select-none">
                              Console{cmd.status === 'running' ? '…' : ''}
                            </summary>
                            <pre className="chat-console-output mt-1">{parsed.console}</pre>
                          </details>
                        )}
                        {parsed.response && (
                          <MarkdownOutput
                            content={parsed.response}
                            failed={cmd.status === 'failed'}
                            scrollable={false}
                            repoPath={repoPath}
                            onFileClick={handleFileClick}
                          />
                        )}
                        {!parsed.response && cmd.status === 'running' && (
                          <p className="chat-running text-[var(--text-muted)] text-sm italic">
                            Running…
                          </p>
                        )}
                      </>
                    ) : cmd.status === 'running' ? (
                      <p className="chat-running text-[var(--text-muted)] text-sm italic">
                        Running…
                      </p>
                    ) : cmd.status === 'pending' ? (
                      <p className="chat-running text-[var(--text-muted)] text-sm italic">
                        Pending…
                      </p>
                    ) : null}
                    {cmd.summary && (
                      <div className="chat-summary mt-2">
                        <span className="font-medium">T:</span>
                        <MarkdownOutput
                          content={cmd.summary}
                          scrollable={false}
                          className="mt-1 markdown-output-compact"
                          repoPath={repoPath}
                          onFileClick={handleFileClick}
                        />
                      </div>
                    )}
                  </>
                )}
              </div>
            )
          })
        )}
      </main>

      <footer className="chat-footer">
        <form ref={formRef} onSubmit={handleSubmit} className="chat-form">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="Message..."
            rows={2}
            className="textarea-control chat-input"
            disabled={!root}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault()
                formRef.current?.requestSubmit()
              }
            }}
          />
          {error && <p className="error-text">{error}</p>}
          <div className="chat-footer-row">
            <div className="chat-models">
              <div className="chat-select-wrap">
                <select
                  value={translatorModel}
                  onChange={(e) => setTranslatorModel(e.target.value)}
                  className="select-control chat-model-select"
                  aria-label="Translator model"
                >
                  {modelOptions.map((m) => (
                    <option key={m} value={m}>{m}</option>
                  ))}
                </select>
                <div className="chat-select-meta">
                  <span className="chat-select-icon" aria-hidden>
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
                  </span>
                  <span className="chat-select-label">Translator</span>
                </div>
              </div>
              <div className="chat-select-wrap">
                <select
                  value={workloadModel}
                  onChange={(e) => setWorkloadModel(e.target.value)}
                  className="select-control chat-model-select"
                  aria-label="Workload model"
                >
                  {modelOptions.map((m) => (
                    <option key={m} value={m}>{m}</option>
                  ))}
                </select>
                <div className="chat-select-meta">
                  <span className="chat-select-icon" aria-hidden>
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"/><path d="M12 1v4M12 19v4M4.22 4.22l2.83 2.83M16.95 16.95l2.83 2.83M1 12h4M19 12h4M4.22 19.78l2.83-2.83M16.95 7.05l2.83-2.83"/></svg>
                  </span>
                  <span className="chat-select-label">Workload</span>
                </div>
              </div>
              <div className="chat-select-wrap">
                <select
                  value={templateId}
                  onChange={(e) => setTemplateId(e.target.value)}
                  className="select-control chat-model-select"
                  aria-label="Template"
                  title="Select preset for this message"
                >
                  {TEMPLATES.map((t) => (
                    <option key={t.id} value={t.id}>{t.label}</option>
                  ))}
                </select>
                <div className="chat-select-meta">
                  <span className="chat-select-icon" aria-hidden>
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/></svg>
                  </span>
                  <span className="chat-select-label">Template</span>
                </div>
              </div>
            </div>
            <button
              type="submit"
              disabled={loading || !input.trim() || !root}
              className="btn btn-primary chat-send"
            >
              {loading ? '…' : 'Send'}
            </button>
          </div>
        </form>
      </footer>
    </div>
  )
}
