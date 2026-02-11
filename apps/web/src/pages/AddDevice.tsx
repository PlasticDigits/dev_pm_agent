import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { reserveCode } from '../api/devices'
import { generateWordCode } from '../utils/wordCode'
import { useAuth } from '../contexts/AuthContext'

export default function AddDevice() {
  const { token } = useAuth()
  const [code, setCode] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const navigate = useNavigate()

  async function handleKeygen() {
    if (!token) {
      setError('Login required')
      return
    }
    setError('')
    setLoading(true)
    try {
      const newCode = generateWordCode()
      await reserveCode(token, newCode)
      setCode(newCode)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to reserve code')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="mobile-frame flex min-h-screen flex-col gap-2 py-3">
      <h1 className="title-main">Add new device</h1>
      <p className="title-sub">Generate a code and run: executor register-device {'<code>'} {'<password>'}</p>

      <div className="panel mt-1 w-full space-y-2.5">
        <button
          onClick={handleKeygen}
          disabled={loading}
          className="btn btn-primary w-full"
        >
          {loading ? 'Generating…' : 'API keygen'}
        </button>
        {code && (
          <div className="panel-muted">
            <p className="text-sm text-muted">Enter this code in executor:</p>
            <p className="mt-2 font-mono text-lg tracking-wider">{code}</p>
          </div>
        )}
        {error && <p className="error-text">{error}</p>}
        <button
          onClick={() => navigate('/chat')}
          className="btn btn-secondary w-full"
        >
          Back to Chat
        </button>
      </div>
    </div>
  )
}
