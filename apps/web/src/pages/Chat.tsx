import { useState, useEffect, useCallback } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { createCommand, listCommands } from '../api/commands'
import { listModels } from '../api/models'
import { listRepos } from '../api/repos'
import { getToken, clearAuth } from '../stores/auth'

const WS_BASE = (() => {
  const u = import.meta.env.VITE_RELAYER_URL || ''
  if (u.startsWith('http')) return u.replace('http', 'ws')
  return `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}`
})()

interface Repo {
  id: string
  path: string
  name?: string
}

export default function Chat() {
  const [token, setToken] = useState<string | null>(getToken())
  const [commands, setCommands] = useState<any[]>([])
  const [repos, setRepos] = useState<Repo[]>([])
  const [models, setModels] = useState<string[]>([])
  const [selectedRepoPath, setSelectedRepoPath] = useState<string>('')
  const [translatorModel, setTranslatorModel] = useState<string>('composer-1.5')
  const [workloadModel, setWorkloadModel] = useState<string>('composer-1.5')
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

  const refreshRepos = useCallback(async () => {
    if (!token) return
    try {
      const list = await listRepos(token)
      setRepos(list)
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

  useEffect(() => {
    if (!token) return
    const wsUrl = `${WS_BASE}/ws?token=${token}`
    const ws = new WebSocket(wsUrl)
    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data as string)
        if (msg.type === 'command_update' && msg.payload) {
          const { id, status, output, summary } = msg.payload
          setCommands((prev) => {
            const idx = prev.findIndex((c) => c.id === id)
            if (idx >= 0) {
              const next = [...prev]
              next[idx] = { ...next[idx], status, output, summary }
              return next
            }
            return prev
          })
          return
        }
      } catch (_) {}
      refreshCommands()
    }
    return () => ws.close()
  }, [token, refreshCommands])

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!token) return
    setError('')
    setLoading(true)
    try {
      await createCommand(token, {
        input,
        repo_path: selectedRepoPath || undefined,
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
    setToken(null)
    navigate('/login')
  }

  if (!token) return null

  return (
    <div className="flex min-h-screen flex-col p-4">
      <div className="mx-auto w-full max-w-2xl">
        <div className="flex items-center justify-between">
          <h1 className="text-2xl font-bold">Dev PM Agent</h1>
          <div className="flex gap-2">
            <button
              onClick={() => navigate('/repos')}
              className="rounded border border-gray-600 px-3 py-1 text-sm hover:bg-gray-800"
            >
              Repos
            </button>
            <button
              onClick={() => navigate('/add-device')}
              className="rounded border border-gray-600 px-3 py-1 text-sm hover:bg-gray-800"
            >
              Add device
            </button>
            <button
              onClick={handleLogout}
              className="rounded border border-gray-600 px-3 py-1 text-sm hover:bg-gray-800"
            >
              Logout
            </button>
          </div>
        </div>

        <form onSubmit={handleSubmit} className="mt-6 space-y-3">
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
            <div>
              <label htmlFor="repo" className="block text-sm text-gray-400">
                Repo
              </label>
              {repos.length === 0 && (
                <p className="mb-1 text-xs text-amber-500">
                  No repos. <Link to="/repos" className="underline hover:no-underline">Add one in Repos</Link>.
                </p>
              )}
              <select
                id="repo"
                value={selectedRepoPath}
                onChange={(e) => setSelectedRepoPath(e.target.value)}
                className="mt-1 w-full rounded border border-gray-600 bg-gray-800 px-3 py-2 text-white"
              >
                <option value="">— None —</option>
                {repos.map((r) => (
                  <option key={r.id} value={r.path}>
                    {r.name || r.path}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label htmlFor="translator" className="block text-sm text-gray-400">
                Translator model
              </label>
              <select
                id="translator"
                value={translatorModel}
                onChange={(e) => setTranslatorModel(e.target.value)}
                className="mt-1 w-full rounded border border-gray-600 bg-gray-800 px-3 py-2 text-white"
              >
                {(models.length ? models : ['composer-1.5']).map((m) => (
                  <option key={m} value={m}>
                    {m}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label htmlFor="workload" className="block text-sm text-gray-400">
                Workload model
              </label>
              <select
                id="workload"
                value={workloadModel}
                onChange={(e) => setWorkloadModel(e.target.value)}
                className="mt-1 w-full rounded border border-gray-600 bg-gray-800 px-3 py-2 text-white"
              >
                {(models.length ? models : ['composer-1.5']).map((m) => (
                  <option key={m} value={m}>
                    {m}
                  </option>
                ))}
              </select>
            </div>
          </div>
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="Describe the task..."
            rows={3}
            className="w-full rounded border border-gray-600 bg-gray-800 px-3 py-2 text-white"
            required
          />
          {error && <p className="text-red-400">{error}</p>}
          <button
            type="submit"
            disabled={loading}
            className="rounded bg-blue-600 px-4 py-2 font-medium hover:bg-blue-700 disabled:opacity-50"
          >
            {loading ? 'Sending…' : 'Send'}
          </button>
        </form>

        <div className="mt-8">
          <h2 className="text-lg font-semibold">Commands</h2>
          <div className="mt-2 space-y-2">
            {commands.length === 0 && (
              <p className="text-gray-400">No commands yet.</p>
            )}
            {commands.map((cmd) => (
              <div
                key={cmd.id}
                className="rounded border border-gray-600 bg-gray-800 p-3"
              >
                <p className="font-medium">{cmd.input}</p>
                <p className="mt-1 text-sm text-gray-400">
                  Status: {cmd.status}
                  {cmd.summary && ` • ${cmd.summary}`}
                </p>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}
