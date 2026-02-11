const BASE = import.meta.env.VITE_RELAYER_URL || '';

function authHeaders(token: string) {
  return {
    'Content-Type': 'application/json',
    Authorization: `Bearer ${token}`,
  };
}

export interface ReadFileResponse {
  content: string
}

export interface FileSearchMatch {
  path: string
  modified_at: string
}

export interface FileSearchResponse {
  matches: FileSearchMatch[]
}

export async function searchFiles(
  token: string,
  repoPath: string,
  fileName: string
): Promise<FileSearchResponse> {
  const params = new URLSearchParams({ repo_path: repoPath, file_name: fileName })
  const res = await fetch(`${BASE}/api/files/search?${params}`, {
    headers: authHeaders(token),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || `Failed to search files: ${res.status}`)
  }
  return res.json()
}

export async function listMdFiles(
  token: string,
  repoPath: string
): Promise<FileSearchResponse> {
  return searchFiles(token, repoPath, '*.md')
}

export async function readFile(
  token: string,
  repoPath: string,
  filePath: string
): Promise<ReadFileResponse> {
  const params = new URLSearchParams({ repo_path: repoPath, file_path: filePath });
  const res = await fetch(`${BASE}/api/files/read?${params}`, {
    headers: authHeaders(token),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(text || `Failed to read file: ${res.status}`);
  }
  return res.json();
}
