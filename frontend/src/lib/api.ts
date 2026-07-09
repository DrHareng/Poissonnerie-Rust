import type {
  ApiError,
  Army,
  AuthUser,
  MatchOutcome,
  MatchRecord,
  Player,
  PlayerProfile,
  PlayerTournamentResult,
  RankedArmy,
  RankedPlayer,
  Scenario,
  Tournament,
  TournamentDetail,
  TournamentListEntry,
  TournamentMatch,
  TournamentRegistration,
} from '@/types/elo'

const defaultFetchOptions: RequestInit = {
  credentials: 'include',
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...defaultFetchOptions,
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

export function fetchArmies(): Promise<Army[]> {
  return request<Army[]>('/api/armies')
}

export function fetchRanking(): Promise<RankedPlayer[]> {
  return request<RankedPlayer[]>('/api/ranking')
}

export function fetchArmyRanking(): Promise<RankedArmy[]> {
  return request<RankedArmy[]>('/api/armies/ranking')
}

export function fetchArmyStats(id: number): Promise<RankedArmy> {
  return request<RankedArmy>(`/api/armies/${id}`)
}

export function fetchArmyMatches(id: number, limit = 50): Promise<MatchRecord[]> {
  return request<MatchRecord[]>(`/api/armies/${id}/matches?limit=${limit}`)
}

export async function fetchMe(): Promise<AuthUser | null> {
  const response = await fetch('/api/auth/me', defaultFetchOptions)
  if (response.status === 401) {
    return null
  }
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
  return (await response.json()) as AuthUser
}

export function loginWithDiscord() {
  window.location.href = '/api/auth/discord'
}

export function logout(): Promise<void> {
  return request<void>('/api/auth/logout', { method: 'POST' })
}

export function updateProfile(payload: {
  local_display_name?: string
  local_avatar_url?: string
  clear_local_display_name?: boolean
  clear_local_avatar_url?: boolean
}): Promise<AuthUser['user']> {
  return request<AuthUser['user']>('/api/auth/me', {
    method: 'PATCH',
    body: JSON.stringify(payload),
  })
}

export function addPlayer(payload: { name: string; discord_username: string }): Promise<Player> {
  return request<Player>('/api/players', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function recordMatch(
  player1: string,
  player2: string,
  outcome: MatchOutcome,
  scores: {
    player1_objectives: number
    player1_survivors: number
    player2_objectives: number
    player2_survivors: number
  },
  armies?: {
    player1_army_id?: number
    player2_army_id?: number
  },
  scenario?: {
    scenario_id?: number
    scenario_name?: string
  },
): Promise<MatchRecord> {
  return request<MatchRecord>('/api/matches', {
    method: 'POST',
    body: JSON.stringify({ player1, player2, outcome, ...scores, ...armies, ...scenario }),
  })
}

export function fetchRecentMatches(limit = 20): Promise<MatchRecord[]> {
  return request<MatchRecord[]>(`/api/matches?limit=${limit}`)
}

export function fetchPlayer(name: string): Promise<PlayerProfile> {
  return request<PlayerProfile>(`/api/players/${encodeURIComponent(name)}`)
}

export function fetchPlayerMatches(name: string, limit = 50): Promise<MatchRecord[]> {
  return request<MatchRecord[]>(
    `/api/players/${encodeURIComponent(name)}/matches?limit=${limit}`,
  )
}

export function fetchScenarios(q?: string, limit = 20): Promise<Scenario[]> {
  const params = new URLSearchParams({ limit: String(limit) })
  if (q?.trim()) params.set('q', q.trim())
  return request<Scenario[]>(`/api/scenarios?${params}`)
}

export function fetchTournaments(): Promise<TournamentListEntry[]> {
  return request<TournamentListEntry[]>('/api/tournaments')
}

export function fetchTournament(id: number): Promise<TournamentDetail> {
  return request<TournamentDetail>(`/api/tournaments/${id}`)
}

export function createTournament(payload: {
  name: string
  bracket_format?: string
}): Promise<Tournament> {
  return request<Tournament>('/api/tournaments', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function openTournamentRegistration(id: number): Promise<Tournament> {
  return request<Tournament>(`/api/tournaments/${id}/open-registration`, { method: 'POST' })
}

export function closeTournamentRegistration(id: number): Promise<Tournament> {
  return request<Tournament>(`/api/tournaments/${id}/close-registration`, { method: 'POST' })
}

export function registerForTournament(
  id: number,
  army_id: number,
): Promise<TournamentRegistration> {
  return request<TournamentRegistration>(`/api/tournaments/${id}/register`, {
    method: 'POST',
    body: JSON.stringify({ army_id }),
  })
}

export function adminRegisterForTournament(
  id: number,
  player_name: string,
  army_id: number,
): Promise<TournamentRegistration> {
  return request<TournamentRegistration>(`/api/tournaments/${id}/registrations`, {
    method: 'POST',
    body: JSON.stringify({ player_name, army_id }),
  })
}

export function reviewRegistration(
  tournamentId: number,
  regId: number,
  action: 'approved' | 'rejected',
): Promise<TournamentRegistration> {
  return request<TournamentRegistration>(
    `/api/tournaments/${tournamentId}/registrations/${regId}/review`,
    { method: 'POST', body: JSON.stringify({ action }) },
  )
}

export function startTournament(id: number): Promise<Tournament> {
  return request<Tournament>(`/api/tournaments/${id}/start`, { method: 'POST' })
}

export function setupTournamentPools(
  id: number,
  pools: { name: string; position: number; players: string[] }[],
): Promise<unknown> {
  return request(`/api/tournaments/${id}/pools`, {
    method: 'POST',
    body: JSON.stringify({ pools }),
  })
}

export function generatePoolMatches(id: number): Promise<TournamentMatch[]> {
  return request<TournamentMatch[]>(`/api/tournaments/${id}/generate-pool-matches`, {
    method: 'POST',
  })
}

export function finalizePools(id: number): Promise<Tournament> {
  return request<Tournament>(`/api/tournaments/${id}/finalize-pools`, { method: 'POST' })
}

export function setupTournamentBracket(
  id: number,
  matches: {
    bracket_slot: number
    player1: string
    player2: string
    quarter_player1?: string
  }[],
): Promise<TournamentMatch[]> {
  return request<TournamentMatch[]>(`/api/tournaments/${id}/setup-bracket`, {
    method: 'POST',
    body: JSON.stringify({ matches }),
  })
}

export function generateBracket(id: number): Promise<TournamentMatch[]> {
  return request<TournamentMatch[]>(`/api/tournaments/${id}/generate-bracket`, {
    method: 'POST',
  })
}

export function submitTournamentMatch(
  matchId: number,
  payload: {
    player1_objectives: number
    player2_objectives: number
    player1_survivors?: number
    player2_survivors?: number
    scenario_name?: string
  },
): Promise<TournamentMatch> {
  return request<TournamentMatch>(`/api/tournament-matches/${matchId}/submit`, {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function confirmTournamentMatch(matchId: number): Promise<TournamentMatch> {
  return request<TournamentMatch>(`/api/tournament-matches/${matchId}/confirm`, {
    method: 'POST',
  })
}

export function forfeitTournamentMatch(
  matchId: number,
  forfeit_player: string,
): Promise<TournamentMatch> {
  return request<TournamentMatch>(`/api/tournament-matches/${matchId}/forfeit`, {
    method: 'POST',
    body: JSON.stringify({ forfeit_player }),
  })
}

export function unplayedTournamentMatch(matchId: number): Promise<TournamentMatch> {
  return request<TournamentMatch>(`/api/tournament-matches/${matchId}/unplayed`, {
    method: 'POST',
  })
}

export function correctTournamentMatch(
  matchId: number,
  payload: {
    player1_objectives: number
    player2_objectives: number
    player1_survivors?: number
    player2_survivors?: number
  },
): Promise<TournamentMatch> {
  return request<TournamentMatch>(`/api/tournament-matches/${matchId}/correct`, {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function fetchPlayerTournaments(name: string): Promise<PlayerTournamentResult[]> {
  return request<PlayerTournamentResult[]>(
    `/api/players/${encodeURIComponent(name)}/tournaments`,
  )
}
