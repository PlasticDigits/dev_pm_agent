const BASE = import.meta.env.VITE_RELAYER_URL || '';

function authHeaders(token: string) {
  return {
    'Content-Type': 'application/json',
    Authorization: `Bearer ${token}`,
  };
}

export async function createCommand(
  token: string,
  data: {
    input: string
    repo_path?: string
    context_mode?: string
    translator_model?: string
    workload_model?: string
    cursor_chat_id?: string
  }
) {
  const res = await fetch(`${BASE}/api/commands`, {
    method: 'POST',
    headers: authHeaders(token),
    body: JSON.stringify(data),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function listCommands(token: string) {
  const res = await fetch(`${BASE}/api/commands`, {
    headers: authHeaders(token),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function deleteCommand(token: string, id: string) {
  const res = await fetch(`${BASE}/api/commands/${id}`, {
    method: 'DELETE',
    headers: authHeaders(token),
  });
  if (!res.ok) throw new Error(await res.text());
}
