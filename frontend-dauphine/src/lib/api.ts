import { withBase } from '@/lib/basePath'

export type EditionStatus =
  | 'draft'
  | 'announced'
  | 'registration'
  | 'live'
  | 'completed'

export type DauphineEdition = {
  id: number
  slug: string
  title: string
  year: number
  status: EditionStatus
  tagline: string
  body_md: string
  created_at: number
  updated_at: number
}

type ApiError = {
  error?: string
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(withBase(path), {
    credentials: 'include',
    headers: {
      'Content-Type': 'application/json',
      ...init?.headers,
    },
    ...init,
  })

  if (!response.ok) {
    let message = `Erreur HTTP ${response.status}`
    try {
      const payload = (await response.json()) as ApiError
      if (payload.error) {
        message = payload.error
      }
    } catch {
      // ignore JSON parse errors
    }
    throw new Error(message)
  }

  if (response.status === 204) {
    return undefined as T
  }

  return (await response.json()) as T
}

export function fetchEditions(): Promise<DauphineEdition[]> {
  return request<DauphineEdition[]>('/api/dauphine/editions')
}
