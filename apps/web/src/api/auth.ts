const BASE = import.meta.env.VITE_RELAYER_URL || '';

export async function setup(username: string, password: string) {
  const res = await fetch(`${BASE}/api/auth/setup`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function login(deviceApiKey: string, password: string, totpCode: string) {
  const res = await fetch(`${BASE}/api/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      device_api_key: deviceApiKey,
      password,
      totp_code: totpCode,
    }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}
