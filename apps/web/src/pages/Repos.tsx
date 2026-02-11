import { useState, useEffect, useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { listRepos, addRepo } from '../api/repos'
import { getToken, clearAuth } from '../stores/auth'

interface Repo {
  id: string
  path: string
  name?: string
  created_at: string
}

export default function Repos() {
  const [token, setToken] = useState<string | null>(getToken())
  const [repos, setRepos] = useState<Repo[]>([])
  const [path, setPath] = useState('')
  const [name, setName] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const navigate = useNavigate()

  const refreshRepos = useCallback(async () => {
    if (!token) return
    try {
      const list = await listRepos(token)
      setRepos(list)
    } catch (_) {}
  }, [token])

  useEffect(() => {
    if (!token) {
      navigate('/login')
      return
    }
    refreshRepos()
  }, [token, navigate, refreshRepos])

  async function handleAdd(e: React.FormEvent) {
    e.preventDefault()
    if (!token) return
    setError('')
    setLoading(true)
    try {
      await addRepo(token, path.trim(), name.trim() || undefined)
      setPath('')
      setName('')
      refreshRepos()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to add repo')
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
          <h1 className="text-2xl font-bold">Repositories</h1>
          <div className="flex gap-2">
            <button
              onClick={() => navigate('/chat')}
              className="rounded border border-gray-600 px-3 py-1 text-sm hover:bg-gray-800"
            >
              Chat
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

        <form onSubmit={handleAdd} className="mt-6 space-y-3">
          <div>
            <label htmlFor="path" className="block text-sm text-gray-400">
              Path (e.g. ~/repos/my-project)
            </label>
            <p className="mt-0.5 text-xs text-gray-500">Path must be under ~/repos/</p>
            <input
              id="path"
              type="text"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder="~/repos/my-project"
              className="mt-1 w-full rounded border border-gray-600 bg-gray-800 px-3 py-2 text-white"
              required
            />
          </div>
          <div>
            <label htmlFor="name" className="block text-sm text-gray-400">
              Name (optional)
            </label>
            <input
              id="name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My Project"
              className="mt-1 w-full rounded border border-gray-600 bg-gray-800 px-3 py-2 text-white"
            />
          </div>
          {error && <p className="text-red-400">{error}</p>}
          <button
            type="submit"
            disabled={loading}
            className="rounded bg-blue-600 px-4 py-2 font-medium hover:bg-blue-700 disabled:opacity-50"
          >
            {loading ? 'Adding…' : 'Add repo'}
          </button>
        </form>

        <div className="mt-8">
          <h2 className="text-lg font-semibold">Your repos</h2>
          <div className="mt-2 space-y-2">
            {repos.length === 0 && (
              <p className="text-gray-400">No repos yet. Add one above.</p>
            )}
            {repos.map((repo) => (
              <div
                key={repo.id}
                className="rounded border border-gray-600 bg-gray-800 p-3"
              >
                <p className="font-medium">{repo.name || repo.path}</p>
                <p className="mt-1 text-sm text-gray-400 font-mono">{repo.path}</p>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}
