const BASE = import.meta.env.VITE_RELAYER_URL || ''

function authHeaders(token: string) {
  return {
    'Content-Type': 'application/json',
    Authorization: `Bearer ${token}`,
  }
}

export async function listModels(token: string): Promise<string[]> {
  const res = await fetch(`${BASE}/api/models`, {
    headers: authHeaders(token),
  })
  if (!res.ok) throw new Error(await res.text())
  return res.json()
}
