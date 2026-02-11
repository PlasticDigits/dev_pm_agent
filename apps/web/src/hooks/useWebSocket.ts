import { useEffect, useRef, useState } from 'react'

const WS_BASE = (() => {
  const u = import.meta.env.VITE_RELAYER_URL || ''
  if (u.startsWith('http')) return u.replace('http', 'ws')
  return `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}`
})()

/**
 * WebSocket with first-message auth, automatic reconnection, and exponential backoff.
 * Connects without token in URL, sends auth as first message,
 * and only forwards command_new/command_update after auth_ok.
 */
export function useWebSocket(
  token: string | null,
  onMessage: (event: MessageEvent) => void
): { ready: boolean; error: string | null } {
  const [ready, setReady] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const onMessageRef = useRef(onMessage)
  onMessageRef.current = onMessage

  useEffect(() => {
    if (!token) return

    let aborted = false
    let ws: WebSocket | null = null
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null
    let attempt = 0
    const MAX_BACKOFF_MS = 15000
    const BASE_BACKOFF_MS = 1000

    function connect() {
      if (aborted) return

      setError(null)
      let authenticated = false

      ws = new WebSocket(`${WS_BASE}/ws`)

      ws.onopen = () => {
        if (aborted) return
        attempt = 0 // reset backoff on successful connect
        ws!.send(JSON.stringify({ type: 'auth', payload: { token } }))
      }

      ws.onmessage = (event) => {
        if (aborted) return
        try {
          const msg = JSON.parse(event.data as string)
          if (msg.type === 'auth_ok') {
            authenticated = true
            setReady(true)
            return
          }
          if (msg.type === 'auth_fail') {
            setError(msg.payload?.reason ?? 'Authentication failed')
            // Don't reconnect on auth failure — token is bad
            return
          }
        } catch {
          // Not JSON or unexpected format — forward to handler if already authenticated
        }
        if (authenticated) {
          onMessageRef.current(event)
        }
      }

      ws.onerror = () => {
        if (!aborted && !authenticated) {
          setError('WebSocket connection failed. Retrying…')
        }
      }

      ws.onclose = () => {
        if (aborted) return
        setReady(false)
        // Reconnect with exponential backoff
        const delay = Math.min(BASE_BACKOFF_MS * Math.pow(2, attempt), MAX_BACKOFF_MS)
        attempt++
        reconnectTimer = setTimeout(connect, delay)
      }
    }

    connect()

    return () => {
      aborted = true
      if (reconnectTimer) clearTimeout(reconnectTimer)
      if (ws) ws.close()
      setReady(false)
    }
  }, [token])

  return { ready, error }
}
