/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_RELAYER_URL: string
  readonly VITE_CLIENT_SALT: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
