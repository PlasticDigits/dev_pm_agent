/**
 * API client base — to be implemented per PLAN.md
 * Uses VITE_RELAYER_URL for production, /api proxy for dev
 */
const BASE_URL = import.meta.env.VITE_RELAYER_URL || '';

export async function apiFetch(path: string, options?: RequestInit): Promise<Response> {
  const url = BASE_URL ? `${BASE_URL}${path}` : path;
  return fetch(url, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...options?.headers,
    },
  });
}
