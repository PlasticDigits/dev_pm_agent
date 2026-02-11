import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { setup } from '../api/auth'
import { setDeviceKey } from '../stores/auth'

export default function Setup() {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const navigate = useNavigate()

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError('')
    setLoading(true)
    try {
      const { totp_secret, device_api_key } = await setup(username, password)
      setDeviceKey(device_api_key)
      alert(
        `Save the device key before leaving — you need it to log in from other devices.\n\n` +
          `Add this TOTP secret to your authenticator app:\n\n${totp_secret}`
      )
      navigate('/login')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Setup failed')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="flex min-h-screen flex-col items-center justify-center p-4">
      <h1 className="text-2xl font-bold">Dev PM Agent — Setup</h1>
      <p className="mt-2 text-gray-400">Create your account (first-run only)</p>

      <form onSubmit={handleSubmit} className="mt-6 w-full max-w-sm space-y-4">
        <div>
          <label className="block text-sm text-gray-400">Username</label>
          <input
            type="text"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
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
        {error && <p className="text-red-400">{error}</p>}
        <button
          type="submit"
          disabled={loading}
          className="w-full rounded bg-blue-600 py-2 font-medium hover:bg-blue-700 disabled:opacity-50"
        >
          {loading ? 'Creating…' : 'Create account'}
        </button>
      </form>
    </div>
  )
}
