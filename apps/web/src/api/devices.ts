const BASE = import.meta.env.VITE_RELAYER_URL || '';

function authHeaders(token: string) {
  return {
    'Content-Type': 'application/json',
    Authorization: `Bearer ${token}`,
  };
}

export async function reserveCode(token: string, code: string) {
  const res = await fetch(`${BASE}/api/devices/reserve-code`, {
    method: 'POST',
    headers: authHeaders(token),
    body: JSON.stringify({ code }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}
