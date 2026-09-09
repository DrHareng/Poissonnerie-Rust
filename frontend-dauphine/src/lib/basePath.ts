/** Base de l'app (`/dauphine/`), toujours avec slash final côté Vite. */
export function appBase(): string {
  return import.meta.env.BASE_URL || '/'
}

/** Préfixe sans slash final (`/dauphine` ou ``). */
export function appBasePrefix(): string {
  return appBase().replace(/\/$/, '')
}

/**
 * Préfixe un chemin absolu de l'app (`/api/...`, `/resources/...`)
 * avec la base Vite. Les URLs http(s) sont laissées telles quelles.
 */
export function withBase(path: string): string {
  if (!path) return appBasePrefix() || '/'
  if (/^https?:\/\//i.test(path) || path.startsWith('data:')) {
    return path
  }
  const normalized = path.startsWith('/') ? path : `/${path}`
  return `${appBasePrefix()}${normalized}`
}
