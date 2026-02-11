const TOKEN_KEY = 'jwt';
const DEVICE_KEY = 'device_api_key';

export function getToken(): string | null {
  return localStorage.getItem(TOKEN_KEY);
}

export function setToken(token: string) {
  localStorage.setItem(TOKEN_KEY, token);
}

export function clearToken() {
  localStorage.removeItem(TOKEN_KEY);
}

export function clearAuth() {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(DEVICE_KEY);
}

export function getDeviceKey(): string | null {
  return localStorage.getItem(DEVICE_KEY);
}

export function setDeviceKey(key: string) {
  localStorage.setItem(DEVICE_KEY, key);
}

export function clearDeviceKey() {
  localStorage.removeItem(DEVICE_KEY);
}
