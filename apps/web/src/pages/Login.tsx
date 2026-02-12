import { useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { login } from '../api/auth'
import { getDeviceKey, setDeviceKey, clearDeviceKey } from '../stores/auth'
import { useAuth } from '../contexts/AuthContext'

export default function Login() {
  const { setToken } = useAuth()
  const [keyCleared, setKeyCleared] = useState(false)
  const savedKey = keyCleared ? null : getDeviceKey()
  const [showDeviceField, setShowDeviceField] = useState(false)
  const [deviceApiKey, setDeviceApiKey] = useState('')
  const [password, setPassword] = useState('')
  const [totpCode, setTotpCode] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const navigate = useNavigate()

  const showDeviceInput = showDeviceField || !savedKey
  const keyToUse = showDeviceInput ? deviceApiKey : savedKey || ''

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError('')
    setLoading(true)
    try {
      const { token } = await login(keyToUse, password, totpCode)
      setToken(token)
      setDeviceKey(keyToUse)
      navigate('/chat')
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Login failed'
      setError(msg)
      if (msg.toLowerCase().includes('invalid device')) {
        clearDeviceKey()
        setKeyCleared(true)
      }
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="mobile-frame flex min-h-screen flex-col gap-2 py-3">
      <h1 className="title-main">Dev PM Agent</h1>
      <p className="title-sub">Login</p>

      <form onSubmit={handleSubmit} className="panel mt-1 w-full space-y-2.5">
        {showDeviceInput ? (
          <div>
            <label className="field-label">Device API key</label>
            <input
              type="text"
              value={deviceApiKey}
              onChange={(e) => setDeviceApiKey(e.target.value)}
              placeholder="From bootstrap-device or register-device CLI"
              className="input-control"
              required
            />
          </div>
        ) : (
          <p className="text-sm text-muted">
            Using saved device key.{' '}
            <button
              type="button"
              onClick={() => setShowDeviceField(true)}
              className="text-sm"
            >
              Use different device
            </button>
            {' • '}
            <button
              type="button"
              onClick={() => {
                clearDeviceKey()
                setKeyCleared(true)
              }}
              className="text-sm"
            >
              Clear and run setup
            </button>
          </p>
        )}
        <div>
          <label className="field-label">Password</label>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            className="input-control"
            required
          />
        </div>
        <div>
          <label className="field-label">TOTP code</label>
          <input
            type="text"
            value={totpCode}
            onChange={(e) => setTotpCode(e.target.value)}
            placeholder="6 digits"
            maxLength={6}
            className="input-control"
            required
          />
        </div>
        {error && <p className="error-text">{error}</p>}
        <button
          type="submit"
          disabled={loading || !keyToUse}
          className="btn btn-primary w-full"
        >
          {loading ? 'Logging in…' : 'Login'}
        </button>
        <p className="text-center text-sm text-muted">
          First time? <Link to="/setup">Setup</Link>
        </p>
      </form>
    </div>
  )
}
