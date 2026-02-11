const BASE = import.meta.env.VITE_RELAYER_URL || '';
const CLIENT_SALT =
  import.meta.env.VITE_CLIENT_SALT || 'dev-pm-agent-default-salt-change-in-env';

/** Hash password client-side before sending. Domain-binding + salt so reused passwords yield different hashes. */
async function hashPassword(password: string): Promise<string> {
  const encoder = new TextEncoder();
  const data = encoder.encode(CLIENT_SALT + ':dev-pm-agent:' + password);
  const hashBuffer = await crypto.subtle.digest('SHA-256', data);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map((b) => b.toString(16).padStart(2, '0')).join('');
}

export async function verifyBootstrap(deviceApiKey: string): Promise<{ valid: boolean }> {
  const res = await fetch(`${BASE}/api/auth/verify-bootstrap`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ device_api_key: deviceApiKey }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function setup(deviceApiKey: string, username: string, password: string) {
  const passwordHash = await hashPassword(password);
  const res = await fetch(`${BASE}/api/auth/setup`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      device_api_key: deviceApiKey,
      username,
      password: passwordHash,
    }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function login(deviceApiKey: string, password: string, totpCode: string) {
  const passwordHash = await hashPassword(password);
  const res = await fetch(`${BASE}/api/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      device_api_key: deviceApiKey,
      password: passwordHash,
      totp_code: totpCode,
    }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function refreshToken(oldToken: string): Promise<{ token: string }> {
  const res = await fetch(`${BASE}/api/auth/refresh`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ token: oldToken }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}
