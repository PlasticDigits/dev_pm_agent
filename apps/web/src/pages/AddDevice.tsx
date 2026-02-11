import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { reserveCode } from '../api/devices'
import { generateWordCode } from '../utils/wordCode'
import { getToken } from '../stores/auth'

export default function AddDevice() {
  const [code, setCode] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const navigate = useNavigate()
  const token = getToken()

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
    <div className="flex min-h-screen flex-col items-center justify-center p-4">
      <h1 className="text-2xl font-bold">Add new device</h1>
      <p className="mt-2 text-gray-400">Generate a code and run: executor register-device {'<code>'} {'<password>'}</p>

      <div className="mt-6 w-full max-w-sm space-y-4">
        <button
          onClick={handleKeygen}
          disabled={loading}
          className="w-full rounded bg-blue-600 py-2 font-medium hover:bg-blue-700 disabled:opacity-50"
        >
          {loading ? 'Generating…' : 'API keygen'}
        </button>
        {code && (
          <div className="rounded border border-gray-600 bg-gray-800 p-4">
            <p className="text-sm text-gray-400">Enter this code in executor:</p>
            <p className="mt-2 font-mono text-lg tracking-wider">{code}</p>
          </div>
        )}
        {error && <p className="text-red-400">{error}</p>}
        <button
          onClick={() => navigate('/chat')}
          className="w-full rounded border border-gray-600 py-2 hover:bg-gray-800"
        >
          Back to Chat
        </button>
      </div>
    </div>
  )
}
