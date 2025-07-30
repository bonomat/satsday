/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_API_BASE_URL: string
  readonly VITE_MIN_BET_SATS: string
  readonly VITE_MAX_BET_SATS: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
