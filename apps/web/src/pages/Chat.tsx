import { useState, useEffect, useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { createCommand, listCommands, deleteCommand } from '../api/commands'
import { useWebSocket } from '../hooks/useWebSocket'
import { listModels } from '../api/models'
import { listRepos } from '../api/repos'
import { useAuth } from '../contexts/AuthContext'
import { TaskConfigSelector, type Repo } from '../components/TaskConfigSelector'
import { TEMPLATES, getTemplateById, getDefaultTemplate } from '../templates'
import type { Command } from '../types'

function folderName(path: string): string {
  const normalized = path.replace(/\/$/, '')
  if (normalized === '~/repos' || normalized.endsWith('/repos')) return '_'
  return normalized.split(/[/\\]/).pop() || path
}

function compactSummary(summary?: string): string | null {
  if (!summary) return null
  const cleaned = summary
    .replace(/[*_`>#]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
  if (!cleaned) return null
  if (cleaned.length <= 180) return cleaned
  return `${cleaned.slice(0, 177)}...`
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
  if (!cleaned) return 'Conversation'
  if (cleaned.length <= 68) return cleaned
  return `${cleaned.slice(0, 65)}...`
}

/// Group commands by cursor_chat_id. Commands with same ID = same thread. Null/undefined = own group.
function groupCommandsIntoThreads(commands: Command[]): Command[][] {
  const byChatId = new Map<string, Command[]>()
  for (const cmd of commands) {
    const key = cmd.cursor_chat_id ?? `cmd:${cmd.id}`
    const list = byChatId.get(key) ?? []
    list.push(cmd)
    byChatId.set(key, list)
  }
  return [...byChatId.values()].map((thread) =>
    thread.sort((a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime())
  )
}

interface ConversationCardProps {
  thread: Command[]
  onDelete: () => void
  onOpen: () => void
}

function ConversationCard({
  thread,
  onDelete,
  onOpen,
}: ConversationCardProps) {
  const root = thread[0]
  const latest = thread[thread.length - 1]
  const title = generatedTitle(thread)
  const summaryPreview = compactSummary(latest.summary)

  return (
    <div className="conversation-row relative">
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation()
          onDelete()
        }}
        className="btn btn-danger absolute right-2 top-2 p-1.5"
        aria-label="Close"
      >
        ×
      </button>
      <button
        type="button"
        onClick={onOpen}
        className="w-full pr-10 text-left"
      >
        <p className="text-[0.98rem] font-semibold leading-6 sm:text-base">{title}</p>
        <p className="mt-1 text-xs font-medium text-muted">
          {folderName(root.repo_path || '~/repos')} • Status: {latest.status}
          {thread.length > 1 && ` • ${thread.length} messages`}
          <span className="ml-2">→</span>
        </p>
        {summaryPreview && (
          <p className="mt-1.5 text-sm leading-6 text-muted">
            {summaryPreview}
          </p>
        )}
      </button>
    </div>
  )
}

export default function Chat() {
  const { token, clearAuth } = useAuth()
  const [commands, setCommands] = useState<Command[]>([])
  const [repos, setRepos] = useState<Repo[]>([])
  const [models, setModels] = useState<string[]>([])
  const [selectedRepoPath, setSelectedRepoPath] = useState<string>('~/repos')
  const [translatorModel, setTranslatorModel] = useState<string>('composer-1.5')
  const [workloadModel, setWorkloadModel] = useState<string>('composer-1.5')
  const [templateId, setTemplateId] = useState<string>(getDefaultTemplate().id)
  const [input, setInput] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const navigate = useNavigate()

  const refreshCommands = useCallback(async () => {
    if (!token) return
    try {
      const list = await listCommands(token)
      setCommands(list)
    } catch (_) {}
  }, [token])

  const ROOT_REPO = '~/repos'

  const refreshRepos = useCallback(async () => {
    if (!token) return
    try {
      const list = await listRepos(token)
      setRepos(list)
      setSelectedRepoPath((prev) => {
        if (!prev) return ROOT_REPO
        if (prev === ROOT_REPO) return ROOT_REPO
        if (list.some((r: Repo) => r.path === prev)) return prev
        return ROOT_REPO
      })
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
    refreshRepos()
    refreshModels()
  }, [token, navigate, refreshCommands, refreshRepos, refreshModels])

  const handleWsMessage = useCallback(
    (event: MessageEvent) => {
      try {
        const msg = JSON.parse(event.data as string)
        if (msg.type === 'command_update' && msg.payload) {
          const { id, status, output, summary, cursor_chat_id } = msg.payload
          setCommands((prev) => {
            const idx = prev.findIndex((c) => c.id === id)
            if (idx >= 0) {
              const next = [...prev]
              const update = { ...next[idx] }
              if (status) update.status = status
              if (output !== undefined) update.output = output
              if (summary !== undefined) update.summary = summary
              if (cursor_chat_id !== undefined) update.cursor_chat_id = cursor_chat_id
              next[idx] = update
              return next
            }
            refreshCommands()
            return prev
          })
          return
        }
      } catch {
        /* ignore */
      }
      refreshCommands()
    },
    [refreshCommands]
  )

  const { ready: wsReady, error: wsError } = useWebSocket(token, handleWsMessage)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!token) return
    setError('')
    setLoading(true)
    try {
      const tpl = getTemplateById(templateId) ?? getDefaultTemplate()
      await createCommand(token, {
        input,
        repo_path: selectedRepoPath || undefined,
        context_mode: tpl.contextMode ?? undefined,
        translator_model: translatorModel || undefined,
        workload_model: workloadModel || undefined,
      })
      setInput('')
      refreshCommands()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed')
    } finally {
      setLoading(false)
    }
  }

  function handleLogout() {
    clearAuth()
    navigate('/login')
  }

  if (!token) return null

  return (
    <div className="mobile-frame flex min-h-screen flex-col gap-2 py-2.5">
      <div className="w-full">
        <div className="panel flex items-start justify-between gap-2">
          <div>
            <h1 className="title-main">Dev PM Agent</h1>
            <p className="title-sub">Compact mobile control plane</p>
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => navigate('/add-device')}
              className="btn btn-secondary"
            >
              Add device
            </button>
            <button
              onClick={handleLogout}
              className="btn btn-ghost"
            >
              Logout
            </button>
          </div>
        </div>

        <form onSubmit={handleSubmit} className="panel mt-2.5 space-y-2.5">
          <TaskConfigSelector
            repos={repos}
            models={models}
            repoPath={selectedRepoPath}
            translatorModel={translatorModel}
            workloadModel={workloadModel}
            onRepoPathChange={setSelectedRepoPath}
            onTranslatorModelChange={setTranslatorModel}
            onWorkloadModelChange={setWorkloadModel}
            showRepoHint
          />
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="Describe the task..."
            rows={3}
            className="textarea-control"
            required
          />
          {error && <p className="error-text">{error}</p>}
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={templateId}
              onChange={(e) => setTemplateId(e.target.value)}
              className="select-control w-auto min-w-[7rem]"
              aria-label="Template"
              title="Select preset for this message"
            >
              {TEMPLATES.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.label}
                </option>
              ))}
            </select>
            <button
              type="submit"
              disabled={loading}
              className="btn btn-primary"
            >
              {loading ? 'Sending…' : 'Send'}
            </button>
          </div>
          {repos.length === 0 && (
            <p className="text-xs warn-text">
              Run the executor to sync ~/repos/ for more repo options
            </p>
          )}
        </form>

        <div className="mt-2.5">
          <h2 className="text-base font-semibold tracking-tight">Conversations</h2>
          {!wsReady && !wsError && (
            <p className="mt-2 text-sm text-muted">Authenticating…</p>
          )}
          {wsError && (
            <p className="mt-2 text-sm text-red-600">{wsError}</p>
          )}
          <div className="mt-2 space-y-2">
            {commands.length === 0 && wsReady && (
              <p className="text-muted">No conversations yet.</p>
            )}
            {groupCommandsIntoThreads(commands).map((thread) => {
              const root = thread[0]
              const threadKey = root.cursor_chat_id ?? root.id
              return (
                <ConversationCard
                  key={threadKey}
                  thread={thread}
                  onDelete={async () => {
                    if (!token) return
                    try {
                      for (const cmd of thread) {
                        await deleteCommand(token, cmd.id)
                      }
                      refreshCommands()
                    } catch (_) {}
                  }}
                  onOpen={() => navigate(`/chat/${threadKey}`)}
                />
              )
            })}
          </div>
        </div>
      </div>
    </div>
  )
}
