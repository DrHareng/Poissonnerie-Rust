import type {
  AdminAccounts,
  ApiError,
  Army,
  ArmyListStatsGroup,
  ArmyPlayerStats,
  AuthUser,
  CommonRule,
  MatchOutcome,
  MatchRecord,
  PaginatedMatches,
  PaginatedReports,
  Player,
  PlayerArmyStats,
  PlayerProfile,
  PlayerTournamentResult,
  RankedArmy,
  RankedPlayer,
  ReportStatus,
  ReportTemplate,
  Scenario,
  ScenarioDetail,
  ScenarioPack,
  ScenarioPackPage,
  SecondaryObjective,
  Tournament,
  TournamentDetail,
  TournamentListEntry,
  TournamentMatch,
  TournamentRegistration,
  TournamentScenarioSlot,
  TtsContentImage,
  TtsMapDetail,
  TtsMapReport,
  TtsMapSummary,
  TtsMapVariant,
  TtsModuleUpdate,
  User,
} from '@/types/elo'
import { withBase } from '@/lib/basePath'
import {
  getNativeSession,
  isNativeApp,
  openNativeDiscordLogin,
  setNativeSession,
} from '@/lib/nativeApp'

const defaultFetchOptions: RequestInit = {
  credentials: 'include',
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const sessionId = getNativeSession()
  const response = await fetch(withBase(path), {
    ...defaultFetchOptions,
    headers: {
      'Content-Type': 'application/json',
      ...(sessionId ? { 'X-Poissonnerie-Session': sessionId } : {}),
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

export function fetchArmyPlayers(id: number): Promise<ArmyPlayerStats[]> {
  return request<ArmyPlayerStats[]>(`/api/armies/${id}/players`)
}

export async function fetchMe(): Promise<AuthUser | null> {
  const sessionId = getNativeSession()
  const response = await fetch(withBase('/api/auth/me'), {
    ...defaultFetchOptions,
    headers: {
      ...(sessionId ? { 'X-Poissonnerie-Session': sessionId } : {}),
    },
  })
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

export async function loginWithDiscord() {
  if (isNativeApp()) {
    await openNativeDiscordLogin(`${withBase('/api/auth/discord')}?mobile=1`)
    return
  }
  window.location.href = withBase('/api/auth/discord')
}

export function logout(): Promise<void> {
  return request<void>('/api/auth/logout', { method: 'POST' }).finally(() => {
    setNativeSession(null)
  })
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

export type SecondaryViewMode = 'liste' | 'cartes'
export type ArmySortMode = 'win_rate' | 'matches'
export type PlayerSortMode = 'elo' | 'win_rate' | 'matches'
export type TournamentCompletedViewMode = 'detailed' | 'compressed'

export interface UserPrefs {
  secondary_view_mode: SecondaryViewMode
  scenario_slug?: string | null
  tts_map_slug?: string | null
  army_sort_mode: ArmySortMode
  player_sort_mode: PlayerSortMode
  tournament_completed_view_mode: TournamentCompletedViewMode
}

export function fetchPrefs(): Promise<UserPrefs> {
  return request<UserPrefs>('/api/prefs')
}

export function updatePrefs(payload: {
  secondary_view_mode?: SecondaryViewMode
  scenario_slug?: string
  tts_map_slug?: string
  army_sort_mode?: ArmySortMode
  player_sort_mode?: PlayerSortMode
  tournament_completed_view_mode?: TournamentCompletedViewMode
}): Promise<UserPrefs> {
  return request<UserPrefs>('/api/prefs', {
    method: 'PATCH',
    body: JSON.stringify(payload),
  })
}

export interface RessourcesContent {
  body_md: string
  updated_at: number
}

export function fetchRessources(): Promise<RessourcesContent> {
  return request<RessourcesContent>('/api/ressources')
}

export function updateRessources(payload: {
  body_md: string
}): Promise<RessourcesContent> {
  return request<RessourcesContent>('/api/ressources', {
    method: 'PATCH',
    body: JSON.stringify(payload),
  })
}

export function fetchTtsMaps(): Promise<TtsMapSummary[]> {
  return request('/api/tts-maps')
}

export function fetchTtsMap(id: number): Promise<TtsMapDetail> {
  return request(`/api/tts-maps/${id}`)
}

export function createTtsMap(name: string): Promise<TtsMapDetail> {
  return request('/api/tts-maps', {
    method: 'POST',
    body: JSON.stringify({ name }),
  })
}

export function renameTtsMap(id: number, name: string): Promise<TtsMapDetail> {
  return request(`/api/tts-maps/${id}`, {
    method: 'PATCH',
    body: JSON.stringify({ name }),
  })
}

export function deleteTtsMap(id: number): Promise<void> {
  return request(`/api/tts-maps/${id}`, { method: 'DELETE' })
}

export function fetchTtsModuleUpdates(): Promise<TtsModuleUpdate[]> {
  return request('/api/tts-module-updates')
}

export function createTtsModuleUpdate(bodyMd: string): Promise<TtsModuleUpdate> {
  return request('/api/tts-module-updates', {
    method: 'POST',
    body: JSON.stringify({ body_md: bodyMd }),
  })
}

export function updateTtsModuleUpdate(
  id: number,
  bodyMd: string,
): Promise<TtsModuleUpdate> {
  return request(`/api/tts-module-updates/${id}`, {
    method: 'PATCH',
    body: JSON.stringify({ body_md: bodyMd }),
  })
}

export function deleteTtsModuleUpdate(id: number): Promise<void> {
  return request(`/api/tts-module-updates/${id}`, { method: 'DELETE' })
}

export function fetchTtsContentImages(): Promise<TtsContentImage[]> {
  return request('/api/tts-map-content-images')
}

async function uploadFile<T>(path: string, file: File): Promise<T> {
  const sessionId = getNativeSession()
  const body = new FormData()
  body.append('file', file)
  const response = await fetch(withBase(path), {
    ...defaultFetchOptions,
    method: 'POST',
    headers: {
      ...(sessionId ? { 'X-Poissonnerie-Session': sessionId } : {}),
    },
    body,
  })
  if (!response.ok) {
    let message = `Erreur HTTP ${response.status}`
    try {
      const payload = (await response.json()) as ApiError
      if (payload.error) message = payload.error
    } catch {
      // ignore JSON parse errors
    }
    throw new Error(message)
  }
  return (await response.json()) as T
}

export function uploadTtsMapJson(id: number, file: File): Promise<TtsMapDetail> {
  return uploadFile(`/api/tts-maps/${id}/json`, file)
}

export function uploadTtsMapPicture(
  id: number,
  file: File,
): Promise<TtsMapDetail> {
  return uploadFile(`/api/tts-maps/${id}/pictures`, file)
}

export function deleteTtsMapPicture(
  mapId: number,
  filename: string,
): Promise<TtsMapDetail> {
  return request(
    `/api/tts-maps/${mapId}/pictures/${encodeURIComponent(filename)}`,
    { method: 'DELETE' },
  )
}

export async function createTtsMapReport(
  mapId: number,
  description: string,
  file?: File | null,
): Promise<TtsMapReport> {
  const sessionId = getNativeSession()
  const body = new FormData()
  body.append('description', description)
  if (file) body.append('file', file)
  const response = await fetch(withBase(`/api/tts-maps/${mapId}/reports`), {
    ...defaultFetchOptions,
    method: 'POST',
    headers: {
      ...(sessionId ? { 'X-Poissonnerie-Session': sessionId } : {}),
    },
    body,
  })
  if (!response.ok) {
    let message = `Erreur HTTP ${response.status}`
    try {
      const payload = (await response.json()) as ApiError
      if (payload.error) message = payload.error
    } catch {
      // ignore JSON parse errors
    }
    throw new Error(message)
  }
  return (await response.json()) as TtsMapReport
}

export function fetchTtsMapReports(): Promise<TtsMapReport[]> {
  return request('/api/tts-map-reports')
}

export function fetchTtsMapReportCount(): Promise<{ count: number }> {
  return request('/api/tts-map-reports/count')
}

export function deleteTtsMapReport(id: number): Promise<void> {
  return request(`/api/tts-map-reports/${id}`, { method: 'DELETE' })
}

export function fetchTtsMapVariants(filters?: {
  mapId?: number
  scenarioId?: number
  tournamentId?: number
}): Promise<TtsMapVariant[]> {
  const params = new URLSearchParams()
  if (filters?.mapId != null) params.set('map_id', String(filters.mapId))
  if (filters?.scenarioId != null) params.set('scenario_id', String(filters.scenarioId))
  if (filters?.tournamentId != null) {
    params.set('tournament_id', String(filters.tournamentId))
  }
  const query = params.toString()
  return request(`/api/tts-map-variants${query ? `?${query}` : ''}`)
}

export async function createTtsMapVariant(payload: {
  mapId: number
  scenarioId: number
  tournamentId?: number | null
  file: File
}): Promise<TtsMapVariant> {
  const sessionId = getNativeSession()
  const body = new FormData()
  body.append('map_id', String(payload.mapId))
  body.append('scenario_id', String(payload.scenarioId))
  if (payload.tournamentId != null) {
    body.append('tournament_id', String(payload.tournamentId))
  }
  body.append('file', payload.file)
  const response = await fetch(withBase('/api/tts-map-variants'), {
    ...defaultFetchOptions,
    method: 'POST',
    headers: {
      ...(sessionId ? { 'X-Poissonnerie-Session': sessionId } : {}),
    },
    body,
  })
  if (!response.ok) {
    let message = `Erreur HTTP ${response.status}`
    try {
      const payload = (await response.json()) as ApiError
      if (payload.error) message = payload.error
    } catch {
      // ignore JSON parse errors
    }
    throw new Error(message)
  }
  return (await response.json()) as TtsMapVariant
}

export function uploadTtsMapVariantJson(
  id: number,
  file: File,
): Promise<TtsMapVariant> {
  return uploadFile(`/api/tts-map-variants/${id}/json`, file)
}

export function deleteTtsMapVariant(id: number): Promise<void> {
  return request(`/api/tts-map-variants/${id}`, { method: 'DELETE' })
}

export function addPlayer(payload: { name: string; discord_username: string }): Promise<Player> {
  return request<Player>('/api/players', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function claimPlayerProfile(payload?: { name?: string }): Promise<Player> {
  return request<Player>('/api/players/me', {
    method: 'POST',
    body: JSON.stringify(payload ?? {}),
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
    scenario_other?: string
  },
): Promise<MatchRecord> {
  return request<MatchRecord>('/api/matches', {
    method: 'POST',
    body: JSON.stringify({ player1, player2, outcome, ...scores, ...armies, ...scenario }),
  })
}

export function fetchRecentMatches(limit = 20, offset = 0): Promise<PaginatedMatches> {
  const params = new URLSearchParams({
    limit: String(limit),
    offset: String(offset),
  })
  return request<PaginatedMatches>(`/api/matches?${params}`)
}

export function fetchMatch(id: number): Promise<MatchRecord> {
  return request<MatchRecord>(`/api/matches/${id}`)
}

export function deleteMatch(id: number): Promise<void> {
  return request<void>(`/api/matches/${id}`, { method: 'DELETE' })
}

export function updateMatchReport(
  id: number,
  body_md: string,
  status: ReportStatus,
): Promise<MatchRecord> {
  return request<MatchRecord>(`/api/matches/${id}/report`, {
    method: 'PATCH',
    body: JSON.stringify({ body_md, status }),
  })
}

export function fetchRecentReports(limit = 5, offset = 0): Promise<PaginatedReports> {
  const params = new URLSearchParams({
    limit: String(limit),
    offset: String(offset),
  })
  return request<PaginatedReports>(`/api/reports/recent?${params}`)
}

export function fetchArmyListArmies(): Promise<number[]> {
  return request<number[]>('/api/army-lists/armies')
}

export function fetchArmyLists(armyIds: number[]): Promise<ArmyListStatsGroup[]> {
  const params = new URLSearchParams()
  if (armyIds.length > 0) {
    params.set('army_ids', armyIds.join(','))
  }
  const query = params.toString()
  return request<ArmyListStatsGroup[]>(
    query ? `/api/army-lists?${query}` : '/api/army-lists',
  )
}

export function fetchArmyListMatches(listId: number, limit = 200): Promise<MatchRecord[]> {
  return request<MatchRecord[]>(`/api/army-lists/${listId}/matches?limit=${limit}`)
}

export function fetchReportTemplates(): Promise<ReportTemplate[]> {
  return request<ReportTemplate[]>('/api/me/report-templates')
}

export function createReportTemplate(payload: {
  name: string
  body_md: string
}): Promise<ReportTemplate> {
  return request<ReportTemplate>('/api/me/report-templates', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function updateReportTemplate(
  id: number,
  payload: { name: string; body_md: string },
): Promise<ReportTemplate> {
  return request<ReportTemplate>(`/api/me/report-templates/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(payload),
  })
}

export function deleteReportTemplate(id: number): Promise<void> {
  return request<void>(`/api/me/report-templates/${id}`, { method: 'DELETE' })
}

export function updateMatchArmyList(
  id: number,
  army_list_code: string,
  army_id?: number | null,
): Promise<MatchRecord> {
  return request<MatchRecord>(`/api/matches/${id}/army-list`, {
    method: 'PATCH',
    body: JSON.stringify({
      army_list_code,
      ...(army_id != null ? { army_id } : {}),
    }),
  })
}

export function startMatch(payload: {
  player1: string
  player2: string
  player1_army_id: number
  player2_army_id: number
  player1_secondary_slugs: string[]
  player2_secondary_slugs: string[]
  counts_for_elo?: boolean
  adversaire?: string
  client_uuid?: string
}): Promise<MatchRecord> {
  return request<MatchRecord>('/api/matches/start', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function syncPartie(payload: {
  client_uuid: string
  player1: string
  player2: string
  adversaire?: string
  player1_army_id: number
  player2_army_id: number
  player1_secondary_slugs?: string[]
  player2_secondary_slugs?: string[]
  counts_for_elo?: boolean
  scenario_id?: number
  scenario_other?: string
  scenario_url?: string
  secondary_pool_slugs?: string[]
  player1_chosen_secondary?: string | null
  player2_chosen_secondary?: string | null
  lieutenant_winner?: string
  lieutenant_winner_choice?: string
  lieutenant_other_choice?: string
  partie_step?: string
  complete?: {
    outcome: MatchOutcome
    player1_objectives: number
    player1_survivors: number
    player2_objectives: number
    player2_survivors: number
  }
}): Promise<MatchRecord> {
  return request<MatchRecord>('/api/matches/sync', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function updateMatchProgress(
  id: number,
  payload: {
    scenario_id?: number
    scenario_other?: string
    scenario_url?: string
    player1_secondary_slugs?: string[]
    player2_secondary_slugs?: string[]
    secondary_pool_slugs?: string[]
    player1_chosen_secondary?: string | null
    player2_chosen_secondary?: string | null
    lieutenant_winner?: string
    lieutenant_winner_choice?: string
    lieutenant_other_choice?: string
    partie_step?: string
    player1_list_slot?: number
    player2_list_slot?: number
  },
): Promise<MatchRecord> {
  return request<MatchRecord>(`/api/matches/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(payload),
  })
}

export function completeMatch(
  id: number,
  payload: {
    outcome: MatchOutcome
    player1_objectives: number
    player1_survivors: number
    player2_objectives: number
    player2_survivors: number
  },
): Promise<MatchRecord> {
  return request<MatchRecord>(`/api/matches/${id}/complete`, {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function correctMatch(
  id: number,
  payload: {
    outcome: MatchOutcome
    player1_objectives: number
    player1_survivors: number
    player2_objectives: number
    player2_survivors: number
  },
): Promise<MatchRecord> {
  return request<MatchRecord>(`/api/matches/${id}/correct`, {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function fetchMyInProgressMatches(): Promise<MatchRecord[]> {
  return request<MatchRecord[]>('/api/matches/mine/in-progress')
}

export function fetchPlayer(name: string): Promise<PlayerProfile> {
  return request<PlayerProfile>(`/api/players/${encodeURIComponent(name)}`)
}

export function fetchPlayerMatches(name: string, limit = 200): Promise<MatchRecord[]> {
  return request<MatchRecord[]>(
    `/api/players/${encodeURIComponent(name)}/matches?limit=${limit}`,
  )
}

export function fetchPlayerArmies(name: string): Promise<PlayerArmyStats[]> {
  return request<PlayerArmyStats[]>(
    `/api/players/${encodeURIComponent(name)}/armies`,
  )
}

export function fetchScenarios(q?: string, limit = 100): Promise<Scenario[]> {
  const params = new URLSearchParams({ limit: String(limit) })
  if (q?.trim()) params.set('q', q.trim())
  return request<Scenario[]>(`/api/scenarios?${params}`)
}

export function fetchScenarioPack(slug: string): Promise<ScenarioPackPage> {
  return request<ScenarioPackPage>(`/api/scenario-packs/${encodeURIComponent(slug)}`)
}

export function fetchPackSecondaries(slug: string): Promise<SecondaryObjective[]> {
  return request<SecondaryObjective[]>(
    `/api/scenario-packs/${encodeURIComponent(slug)}/secondaries`,
  )
}

export function fetchPackCommonRules(slug: string): Promise<CommonRule[]> {
  return request<CommonRule[]>(
    `/api/scenario-packs/${encodeURIComponent(slug)}/common-rules`,
  )
}

export function fetchScenarioContentImages(): Promise<string[]> {
  return request<string[]>('/api/scenario-content-images')
}

export function fetchPackScenario(
  packSlug: string,
  scenarioSlug: string,
): Promise<ScenarioDetail> {
  return request<ScenarioDetail>(
    `/api/scenario-packs/${encodeURIComponent(packSlug)}/scenarios/${encodeURIComponent(scenarioSlug)}`,
  )
}

export function updateScenarioPack(
  slug: string,
  payload: { preamble_md: string },
): Promise<ScenarioPack> {
  return request<ScenarioPack>(`/api/scenario-packs/${encodeURIComponent(slug)}`, {
    method: 'PATCH',
    body: JSON.stringify(payload),
  })
}

export function updatePackSecondary(
  packSlug: string,
  secondarySlug: string,
  payload: { name: string; body_md: string },
): Promise<SecondaryObjective> {
  return request<SecondaryObjective>(
    `/api/scenario-packs/${encodeURIComponent(packSlug)}/secondaries/${encodeURIComponent(secondarySlug)}`,
    {
      method: 'PATCH',
      body: JSON.stringify(payload),
    },
  )
}

export function updatePackCommonRule(
  packSlug: string,
  ruleSlug: string,
  payload: { name: string; body_md: string },
): Promise<CommonRule> {
  return request<CommonRule>(
    `/api/scenario-packs/${encodeURIComponent(packSlug)}/common-rules/${encodeURIComponent(ruleSlug)}`,
    {
      method: 'PATCH',
      body: JSON.stringify(payload),
    },
  )
}

export function updatePackScenario(
  packSlug: string,
  scenarioSlug: string,
  payload: {
    flavor_text?: string
    end_condition_md?: string
    objectives_md?: string
    deployment_notes_md?: string
    exclusion_zones_md?: string
    elements_md?: string
    special_rules_md?: string
  },
): Promise<ScenarioDetail> {
  return request<ScenarioDetail>(
    `/api/scenario-packs/${encodeURIComponent(packSlug)}/scenarios/${encodeURIComponent(scenarioSlug)}`,
    {
      method: 'PATCH',
      body: JSON.stringify(payload),
    },
  )
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

export function deleteTournament(id: number): Promise<void> {
  return request<void>(`/api/tournaments/${id}`, { method: 'DELETE' })
}

export function updateTournamentDetails(
  id: number,
  payload: { name: string; description: string },
): Promise<Tournament> {
  return request<Tournament>(`/api/tournaments/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(payload),
  })
}

export function updateTournamentFormat(
  id: number,
  payload: {
    structure: string
    swiss_rounds?: number
    pool_count?: number
    qualified_per_pool?: number
  },
): Promise<Tournament> {
  return request<Tournament>(`/api/tournaments/${id}/format`, {
    method: 'PATCH',
    body: JSON.stringify(payload),
  })
}

export function openTournamentRegistration(id: number): Promise<Tournament> {
  return request<Tournament>(`/api/tournaments/${id}/open-registration`, { method: 'POST' })
}

export function closeTournamentRegistration(id: number): Promise<Tournament> {
  return request<Tournament>(`/api/tournaments/${id}/close-registration`, { method: 'POST' })
}

export function registerForTournament(id: number): Promise<TournamentRegistration> {
  return request<TournamentRegistration>(`/api/tournaments/${id}/register`, {
    method: 'POST',
    body: JSON.stringify({}),
  })
}

export function completeTournamentRegistrationLists(
  id: number,
  army_list_1: string,
  army_list_2 = '',
): Promise<TournamentRegistration> {
  return request<TournamentRegistration>(`/api/tournaments/${id}/register/lists`, {
    method: 'POST',
    body: JSON.stringify({ army_list_1, army_list_2 }),
  })
}

export function unregisterFromTournament(id: number): Promise<void> {
  return request<void>(`/api/tournaments/${id}/unregister`, { method: 'POST' })
}

export function adminRegisterForTournament(
  id: number,
  player_name: string,
  army_list_1: string,
  army_list_2 = '',
  army_id?: number,
  pool_id?: number,
): Promise<TournamentRegistration> {
  return request<TournamentRegistration>(`/api/tournaments/${id}/registrations`, {
    method: 'POST',
    body: JSON.stringify({
      player_name,
      army_list_1,
      army_list_2,
      ...(army_id != null ? { army_id } : {}),
      ...(pool_id != null ? { pool_id } : {}),
    }),
  })
}

export function reviewRegistration(
  tournamentId: number,
  regId: number,
  action: 'approved' | 'rejected',
  list_slot: 1 | 2,
): Promise<TournamentRegistration> {
  return request<TournamentRegistration>(
    `/api/tournaments/${tournamentId}/registrations/${regId}/review`,
    { method: 'POST', body: JSON.stringify({ action, list_slot }) },
  )
}

export function setTournamentListValidator(
  id: number,
  userId: number | null,
): Promise<Tournament> {
  return request<Tournament>(`/api/tournaments/${id}/list-validator`, {
    method: 'POST',
    body: JSON.stringify({ user_id: userId }),
  })
}

export function fetchUsers(): Promise<User[]> {
  return request<User[]>('/api/users')
}

export function fetchAdminAccounts(): Promise<AdminAccounts> {
  return request<AdminAccounts>('/api/admin/accounts')
}

export function linkPlayerAccount(
  playerName: string,
  userId: number | null,
): Promise<AdminAccounts> {
  return request<AdminAccounts>('/api/admin/player-link', {
    method: 'POST',
    body: JSON.stringify({
      player_name: playerName,
      user_id: userId,
    }),
  })
}

export function deleteUnusedPlayer(playerName: string): Promise<AdminAccounts> {
  return request<AdminAccounts>(
    `/api/admin/players/${encodeURIComponent(playerName)}`,
    { method: 'DELETE' },
  )
}

export function mergePlayers(keep: string, alias: string): Promise<AdminAccounts> {
  return request<AdminAccounts>('/api/admin/players/merge', {
    method: 'POST',
    body: JSON.stringify({ keep, alias }),
  })
}

export function startTournament(id: number): Promise<Tournament> {
  return request<Tournament>(`/api/tournaments/${id}/start`, { method: 'POST' })
}

export function unstartTournament(id: number): Promise<Tournament> {
  return request<Tournament>(`/api/tournaments/${id}/unstart`, { method: 'POST' })
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

export function drawTournamentPools(id: number): Promise<unknown> {
  return request(`/api/tournaments/${id}/draw-pools`, { method: 'POST' })
}

export function setPoolScenarios(
  id: number,
  scenario_ids: number[],
): Promise<TournamentScenarioSlot[]> {
  return request<TournamentScenarioSlot[]>(`/api/tournaments/${id}/pool-scenarios`, {
    method: 'POST',
    body: JSON.stringify({ scenario_ids }),
  })
}

export function drawPoolScenarios(id: number): Promise<TournamentScenarioSlot[]> {
  return request<TournamentScenarioSlot[]>(`/api/tournaments/${id}/pool-scenarios/draw`, {
    method: 'POST',
  })
}

export function rerollPoolScenario(
  id: number,
  slot: string,
): Promise<TournamentScenarioSlot[]> {
  return request<TournamentScenarioSlot[]>(`/api/tournaments/${id}/pool-scenarios/reroll`, {
    method: 'POST',
    body: JSON.stringify({ slot }),
  })
}

export function setBracketScenarioPool(
  id: number,
  scenario_ids: number[],
): Promise<TournamentScenarioSlot[]> {
  return request<TournamentScenarioSlot[]>(`/api/tournaments/${id}/bracket-scenarios`, {
    method: 'POST',
    body: JSON.stringify({ scenario_ids }),
  })
}

export function drawBracketScenarioPool(id: number): Promise<TournamentScenarioSlot[]> {
  return request<TournamentScenarioSlot[]>(`/api/tournaments/${id}/bracket-scenarios/draw`, {
    method: 'POST',
  })
}

export function rerollBracketScenarioPoolSlot(
  id: number,
  slot: string,
): Promise<TournamentScenarioSlot[]> {
  return request<TournamentScenarioSlot[]>(`/api/tournaments/${id}/bracket-scenarios/reroll`, {
    method: 'POST',
    body: JSON.stringify({ slot }),
  })
}

export function assignBracketScenarios(id: number): Promise<TournamentScenarioSlot[]> {
  return request<TournamentScenarioSlot[]>(`/api/tournaments/${id}/bracket-scenarios/assign`, {
    method: 'POST',
  })
}

export function updateMyBracketLists(
  id: number,
  bracket_list_1: string,
  bracket_list_2: string,
): Promise<TournamentRegistration> {
  return request<TournamentRegistration>(`/api/tournaments/${id}/bracket-lists`, {
    method: 'POST',
    body: JSON.stringify({ bracket_list_1, bracket_list_2 }),
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
    player1_list_slot: number
    player2_list_slot: number
    scenario_id?: number
    scenario_other?: string
  },
): Promise<TournamentMatch> {
  return request<TournamentMatch>(`/api/tournament-matches/${matchId}/submit`, {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function startTournamentPartie(matchId: number): Promise<MatchRecord> {
  return request<MatchRecord>(`/api/tournament-matches/${matchId}/start-partie`, {
    method: 'POST',
  })
}

export function submitTournamentFromPartie(
  matchId: number,
  payload: {
    player1_objectives: number
    player2_objectives: number
    player1_survivors?: number
    player2_survivors?: number
    player1_list_slot: number
    player2_list_slot: number
  },
): Promise<TournamentMatch> {
  return request<TournamentMatch>(`/api/tournament-matches/${matchId}/submit-from-partie`, {
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

export function cancelTournamentForfeit(matchId: number): Promise<TournamentMatch> {
  return request<TournamentMatch>(`/api/tournament-matches/${matchId}/cancel-forfeit`, {
    method: 'POST',
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
    player1_list_slot?: number
    player2_list_slot?: number
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
