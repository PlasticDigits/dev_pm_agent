import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { setup, verifyBootstrap } from '../api/auth'
import { setDeviceKey } from '../stores/auth'

export default function Setup() {
  const [step, setStep] = useState<'device' | 'account' | 'totp'>('device')
  const [deviceApiKey, setDeviceApiKey] = useState('')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [totpSecret, setTotpSecret] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const navigate = useNavigate()

  async function handleVerifyDevice(e: React.FormEvent) {
    e.preventDefault()
    setError('')
    setLoading(true)
    try {
      const { valid } = await verifyBootstrap(deviceApiKey.trim())
      if (valid) setStep('account')
      else setError('Device key not found. Run bootstrap-device first.')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Verification failed')
    } finally {
      setLoading(false)
    }
  }

  async function handleCreateAccount(e: React.FormEvent) {
    e.preventDefault()
    setError('')
    setLoading(true)
    try {
      const { totp_secret } = await setup(deviceApiKey.trim(), username, password)
      setTotpSecret(totp_secret)
      setStep('totp')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Setup failed')
    } finally {
      setLoading(false)
    }
  }

  // Step 1: Get device key via CLI, then verify it
  if (step === 'device') {
    return (
      <div className="mobile-frame flex min-h-screen flex-col gap-2 py-3">
        <h1 className="title-main">Dev PM Agent - Setup (Step 1)</h1>
        <p className="title-sub">Register a device key via CLI first</p>
        <div className="panel mt-1 w-full space-y-2.5">
          <p className="text-sm text-muted">
            Run this in your terminal (relayer must be running):
          </p>
          <code className="code-block ok-text">
            source .env && cargo run -p executor -- bootstrap-device
          </code>
          <p className="text-sm text-muted">
            Copy the device key, then paste it below.
          </p>
          <form onSubmit={handleVerifyDevice} className="space-y-2.5">
            <div>
              <label className="field-label">Device key</label>
              <input
                type="text"
                value={deviceApiKey}
                onChange={(e) => setDeviceApiKey(e.target.value)}
                placeholder="Paste the key from CLI output"
                className="input-control"
                required
              />
            </div>
            {error && <p className="error-text">{error}</p>}
            <button
              type="submit"
              disabled={loading}
              className="btn btn-primary w-full"
            >
              {loading ? 'Verifying…' : 'Verify and continue'}
            </button>
          </form>
        </div>
      </div>
    )
  }

  // Step 2: Create account (username, password)
  if (step === 'account') {
    return (
      <div className="mobile-frame flex min-h-screen flex-col gap-2 py-3">
        <h1 className="title-main">Dev PM Agent - Setup (Step 2)</h1>
        <p className="title-sub">Create your account</p>
        <form onSubmit={handleCreateAccount} className="panel mt-1 w-full space-y-2.5">
          <div>
            <label className="field-label">Username</label>
            <input
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              className="input-control"
              required
            />
          </div>
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
          {error && <p className="error-text">{error}</p>}
          <button
            type="submit"
            disabled={loading}
            className="btn btn-primary w-full"
          >
            {loading ? 'Creating…' : 'Create account'}
          </button>
        </form>
      </div>
    )
  }

  // Step 3: Show TOTP secret (no device key — user already has it from step 1)
  return (
    <div className="mobile-frame flex min-h-screen flex-col gap-2 py-3">
      <h1 className="title-main">Dev PM Agent - Save TOTP</h1>
      <p className="title-sub warn-text">
        Add this to your authenticator app. You need it for login.
      </p>
      <div className="panel mt-1 w-full space-y-2.5">
        <div>
          <label className="field-label">
            TOTP secret — add to authenticator (e.g. Dev PM Agent)
          </label>
          <code className="code-block mt-1 block break-all warn-text">
            {totpSecret}
          </code>
          <button
            type="button"
            onClick={() => navigator.clipboard?.writeText(totpSecret)}
            className="btn btn-ghost mt-1"
          >
            Copy
          </button>
        </div>
        <p className="text-sm text-muted">
          For login you need: device key (from step 1), password, and TOTP code.
        </p>
        <button
          type="button"
          onClick={() => {
            setDeviceKey(deviceApiKey.trim())
            navigate('/login')
          }}
          className="btn btn-primary w-full"
        >
          Continue to Login
        </button>
      </div>
    </div>
  )
}
