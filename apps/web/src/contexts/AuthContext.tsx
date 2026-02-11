import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import { refreshToken } from '../api/auth'
import { getToken, setToken as storeSetToken, clearToken } from '../stores/auth'

/** Parse JWT payload to get exp (seconds since epoch). Returns null if invalid. */
function getJwtExp(token: string): number | null {
  try {
    const parts = token.split('.')
    if (parts.length !== 3) return null
    const base64 = parts[1].replace(/-/g, '+').replace(/_/g, '/')
    const padLen = (4 - (base64.length % 4)) % 4
    const padded = padLen ? base64 + '===='.slice(0, padLen) : base64
    const payload = JSON.parse(atob(padded))
    return typeof payload.exp === 'number' ? payload.exp : null
  } catch {
    return null
  }
}

interface AuthContextValue {
  token: string | null
  setToken: (token: string | null) => void
  clearAuth: () => void
}

const AuthContext = createContext<AuthContextValue | null>(null)

/** Refresh token 60 seconds before it expires. */
const REFRESH_BEFORE_SECS = 60

export function AuthProvider({ children }: { children: ReactNode }) {
  const [token, setTokenState] = useState<string | null>(getToken)
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const setToken = useCallback((t: string | null) => {
    setTokenState(t)
    if (t) storeSetToken(t)
    else clearToken()
  }, [])

  const clearAuth = useCallback(() => {
    setTokenState(null)
    clearToken()
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current)
      timeoutRef.current = null
    }
  }, [])

  const scheduleRefresh = useCallback(
    (t: string) => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current)
        timeoutRef.current = null
      }
      const exp = getJwtExp(t)
      if (!exp) return
      const nowSecs = Math.floor(Date.now() / 1000)
      const refreshAt = exp - REFRESH_BEFORE_SECS
      const delayMs = Math.max(0, (refreshAt - nowSecs) * 1000)
      timeoutRef.current = setTimeout(async () => {
        timeoutRef.current = null
        try {
          const { token: newToken } = await refreshToken(t)
          setToken(newToken)
          /* useEffect will re-run and schedule next refresh */
        } catch {
          clearAuth()
        }
      }, delayMs)
    },
    [clearAuth, setToken]
  )

  useEffect(() => {
    if (token) {
      scheduleRefresh(token)
    }
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current)
        timeoutRef.current = null
      }
    }
  }, [token, scheduleRefresh])

  return (
    <AuthContext.Provider value={{ token, setToken, clearAuth }}>
      {children}
    </AuthContext.Provider>
  )
}

export function useAuth() {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error('useAuth must be used within AuthProvider')
  return ctx
}
