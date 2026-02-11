import { useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { login } from '../api/auth'
import { getDeviceKey, setToken, setDeviceKey } from '../stores/auth'

export default function Login() {
  const [deviceApiKey, setDeviceApiKey] = useState(getDeviceKey() || '')
  const [password, setPassword] = useState('')
  const [totpCode, setTotpCode] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const navigate = useNavigate()

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError('')
    setLoading(true)
    try {
      const { token } = await login(deviceApiKey, password, totpCode)
      setToken(token)
      setDeviceKey(deviceApiKey)
      navigate('/chat')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="flex min-h-screen flex-col items-center justify-center p-4">
      <h1 className="text-2xl font-bold">Dev PM Agent</h1>
      <p className="mt-2 text-gray-400">Login</p>

      <form onSubmit={handleSubmit} className="mt-6 w-full max-w-sm space-y-4">
        <div>
          <label className="block text-sm text-gray-400">Device API key</label>
          <input
            type="text"
            value={deviceApiKey}
            onChange={(e) => setDeviceApiKey(e.target.value)}
            placeholder="From setup or device registration"
            className="mt-1 w-full rounded border border-gray-600 bg-gray-800 px-3 py-2 text-white"
            required
          />
        </div>
        <div>
          <label className="block text-sm text-gray-400">Password</label>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            className="mt-1 w-full rounded border border-gray-600 bg-gray-800 px-3 py-2 text-white"
            required
          />
        </div>
        <div>
          <label className="block text-sm text-gray-400">TOTP code</label>
          <input
            type="text"
            value={totpCode}
            onChange={(e) => setTotpCode(e.target.value)}
            placeholder="6 digits"
            maxLength={6}
            className="mt-1 w-full rounded border border-gray-600 bg-gray-800 px-3 py-2 text-white"
            required
          />
        </div>
        {error && <p className="text-red-400">{error}</p>}
        <button
          type="submit"
          disabled={loading}
          className="w-full rounded bg-blue-600 py-2 font-medium hover:bg-blue-700 disabled:opacity-50"
        >
          {loading ? 'Logging in…' : 'Login'}
        </button>
        <p className="text-center text-sm text-gray-400">
          First time? <Link to="/setup" className="text-blue-400 hover:underline">Setup</Link>
        </p>
      </form>
    </div>
  )
}
