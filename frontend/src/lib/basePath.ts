/** Base de l'app (ex. `/infinity/`), toujours avec slash final côté Vite. */
export function appBase(): string {
  return import.meta.env.BASE_URL || '/'
}

/** Préfixe sans slash final (`/infinity` ou ``). */
export function appBasePrefix(): string {
  return appBase().replace(/\/$/, '')
}

const API_ORIGIN_STORAGE_KEY = 'poissonnerie.api-origin'

export function getApiOrigin(): string {
  try {
    const stored = localStorage.getItem(API_ORIGIN_STORAGE_KEY)?.trim()
    if (stored) return stored.replace(/\/$/, '')
  } catch {
    /* private mode */
  }
  const fromEnv = import.meta.env.VITE_API_ORIGIN?.trim()
  if (fromEnv) return fromEnv.replace(/\/$/, '')
  return appBasePrefix()
}

export function setApiOrigin(origin: string) {
  const normalized = origin.trim().replace(/\/$/, '')
  if (normalized) {
    localStorage.setItem(API_ORIGIN_STORAGE_KEY, normalized)
  } else {
    localStorage.removeItem(API_ORIGIN_STORAGE_KEY)
  }
}

/**
 * Préfixe un chemin absolu de l'app (`/api/...`, `/brand/...`)
 * avec la base Vite. Les URLs http(s) sont laissées telles quelles.
 * Les appels `/api/...` passent par `VITE_API_ORIGIN` (APK) si défini.
 */
export function withBase(path: string): string {
  if (!path) return appBasePrefix() || '/'
  if (/^https?:\/\//i.test(path) || path.startsWith('data:')) {
    return path
  }
  const normalized = path.startsWith('/') ? path : `/${path}`
  if (normalized === '/api' || normalized.startsWith('/api/')) {
    return `${getApiOrigin()}${normalized}`
  }
  return `${appBasePrefix()}${normalized}`
}
