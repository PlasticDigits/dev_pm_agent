export const THEMES = [
  {
    id: 'ink',
    name: 'Ink Utility',
    note: 'Balanced contrast and compact spacing',
  },
  {
    id: 'oled',
    name: 'OLED Night',
    note: 'Pure black background for low-light readability',
  },
  {
    id: 'amber',
    name: 'Amber Console',
    note: 'Warm console-like palette with high text clarity',
  },
  {
    id: 'violet',
    name: 'Violet Dense',
    note: 'Soft purple emphasis with rounded touch targets',
  },
  {
    id: 'mint',
    name: 'Mint Matrix',
    note: 'Green-forward contrast for status visibility',
  },
] as const

export type ThemeId = (typeof THEMES)[number]['id']

const DEFAULT_THEME: ThemeId = 'ink'

const THEME_KEY = 'devpm-theme'

export function getStoredTheme(): ThemeId {
  if (typeof window === 'undefined') return DEFAULT_THEME
  const value = window.localStorage.getItem(THEME_KEY) as ThemeId | null
  if (!value || !THEMES.some((theme) => theme.id === value)) return DEFAULT_THEME
  return value
}

export function setStoredTheme(theme: ThemeId) {
  if (typeof window === 'undefined') return
  window.localStorage.setItem(THEME_KEY, theme)
}

export function applyTheme(theme: ThemeId) {
  if (typeof document === 'undefined') return
  document.documentElement.dataset.theme = theme
}

export function initializeTheme() {
  applyTheme(getStoredTheme())
}
