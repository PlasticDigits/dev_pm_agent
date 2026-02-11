const BASE = import.meta.env.VITE_RELAYER_URL || '';

function authHeaders(token: string) {
  return {
    'Content-Type': 'application/json',
    Authorization: `Bearer ${token}`,
  };
}

export async function listRepos(token: string) {
  const res = await fetch(`${BASE}/api/repos`, {
    headers: authHeaders(token),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function addRepo(token: string, path: string, name?: string) {
  const res = await fetch(`${BASE}/api/repos`, {
    method: 'POST',
    headers: authHeaders(token),
    body: JSON.stringify({ path, name }),
  });
  if (!res.ok) throw new Error(await res.text());
}
