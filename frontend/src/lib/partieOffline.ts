import type { PartieLieutenant } from '@/lib/lieutenantRoll'
import type {
  PartieScenario,
  PartieScores,
  PartieStep,
  SecondaryDrawMode,
} from '@/composables/usePartieFlow'
import type {
  Army,
  MatchOutcome,
  MatchRecord,
  RankedPlayer,
  ScenarioSummary,
  SecondaryObjective,
} from '@/types/elo'
import {
  completeMatch,
  syncPartie,
  updateMatchProgress,
} from '@/lib/api'

export const COUPE_REQUIRES_NETWORK =
  'Les parties de coupe nécessitent une connexion réseau.'

const OUTBOX_KEY = 'poissonnerie.partie-outbox'
const CATALOG_KEY = 'poissonnerie.partie-catalog'
const CLIENT_UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

export type LocalPartieComplete = {
  outcome: MatchOutcome
  player1_objectives: number
  player1_survivors: number
  player2_objectives: number
  player2_survivors: number
}

export type LocalPartieDraft = {
  client_uuid: string
  server_id: number | null
  player1: string
  player2: string
  adversaire?: string
  player1_army_id: number
  player2_army_id: number
  counts_for_elo: boolean
  secondary_draw_mode: SecondaryDrawMode
  player1_secondary_slugs: string[]
  player2_secondary_slugs: string[]
  secondary_pool_slugs: string[]
  player1_chosen_secondary: string | null
  player2_chosen_secondary: string | null
  scenario: PartieScenario | null
  lieutenant: PartieLieutenant | null
  scores: PartieScores
  partie_step: PartieStep
  complete?: LocalPartieComplete
  updated_at: number
}

export type PartieCatalogCache = {
  players: RankedPlayer[]
  armies: Army[]
  scenarios: ScenarioSummary[]
  secondaries: SecondaryObjective[]
  saved_at: number
}

export function isClientUuid(value: unknown): value is string {
  return typeof value === 'string' && CLIENT_UUID_RE.test(value.trim())
}

export function newClientUuid(): string {
  return crypto.randomUUID()
}

export function partieResumeParam(match: Pick<MatchRecord, 'id' | 'client_uuid'>): string {
  if (match.id > 0) return String(match.id)
  if (match.client_uuid && isClientUuid(match.client_uuid)) return match.client_uuid
  return String(match.id)
}

function readOutbox(): LocalPartieDraft[] {
  try {
    const raw = localStorage.getItem(OUTBOX_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as unknown
    if (!Array.isArray(parsed)) return []
    return parsed.filter((item): item is LocalPartieDraft => {
      return Boolean(
        item &&
          typeof item === 'object' &&
          isClientUuid((item as LocalPartieDraft).client_uuid),
      )
    })
  } catch {
    return []
  }
}

function writeOutbox(items: LocalPartieDraft[]) {
  localStorage.setItem(OUTBOX_KEY, JSON.stringify(items))
}

export function listLocalParties(): LocalPartieDraft[] {
  return readOutbox().sort((a, b) => b.updated_at - a.updated_at)
}

export function getLocalPartie(clientUuid: string): LocalPartieDraft | null {
  const uuid = clientUuid.trim().toLowerCase()
  return (
    readOutbox().find((item) => item.client_uuid.toLowerCase() === uuid) ?? null
  )
}

export function upsertLocalPartie(draft: LocalPartieDraft): LocalPartieDraft {
  const next: LocalPartieDraft = {
    ...draft,
    client_uuid: draft.client_uuid.trim().toLowerCase(),
    updated_at: Date.now(),
  }
  const items = readOutbox().filter(
    (item) => item.client_uuid.toLowerCase() !== next.client_uuid,
  )
  items.push(next)
  writeOutbox(items)
  return next
}

export function removeLocalPartie(clientUuid: string) {
  const uuid = clientUuid.trim().toLowerCase()
  writeOutbox(readOutbox().filter((item) => item.client_uuid.toLowerCase() !== uuid))
}

export function savePartieCatalog(catalog: Omit<PartieCatalogCache, 'saved_at'>) {
  const payload: PartieCatalogCache = { ...catalog, saved_at: Date.now() }
  localStorage.setItem(CATALOG_KEY, JSON.stringify(payload))
}

export function loadPartieCatalog(): PartieCatalogCache | null {
  try {
    const raw = localStorage.getItem(CATALOG_KEY)
    if (!raw) return null
    const parsed = JSON.parse(raw) as PartieCatalogCache
    if (!Array.isArray(parsed.players) || !Array.isArray(parsed.armies)) return null
    return parsed
  } catch {
    return null
  }
}

export function localPartieToMatchRecord(draft: LocalPartieDraft): MatchRecord {
  return {
    id: draft.server_id ?? 0,
    client_uuid: draft.client_uuid,
    player1: draft.player1,
    player2: draft.player2,
    player1_display_name: draft.player1,
    player2_display_name: draft.player2,
    status: 'in_progress',
    outcome: draft.complete?.outcome ?? null,
    player1_old: 0,
    player1_new: 0,
    player2_old: 0,
    player2_new: 0,
    player1_objectives: draft.complete?.player1_objectives ?? draft.scores.player1Objectives,
    player1_survivors: draft.complete?.player1_survivors ?? draft.scores.player1Survivors,
    player2_objectives: draft.complete?.player2_objectives ?? draft.scores.player2Objectives,
    player2_survivors: draft.complete?.player2_survivors ?? draft.scores.player2Survivors,
    player1_army_id: draft.player1_army_id,
    player2_army_id: draft.player2_army_id,
    scenario_id: draft.scenario?.id ?? null,
    scenario_other: draft.scenario?.other ?? null,
    scenario_url: draft.scenario?.url ?? null,
    scenario_name: draft.scenario?.name ?? draft.scenario?.other ?? null,
    partie_step: draft.complete ? 'resultat' : draft.partie_step,
    counts_for_elo: draft.counts_for_elo,
    adversaire: draft.adversaire ?? null,
    recorded_at: Math.floor(draft.updated_at / 1000),
    player1_secondary_slugs: draft.player1_secondary_slugs,
    player2_secondary_slugs: draft.player2_secondary_slugs,
    secondary_pool_slugs: draft.secondary_pool_slugs,
    player1_chosen_secondary: draft.player1_chosen_secondary,
    player2_chosen_secondary: draft.player2_chosen_secondary,
    lieutenant_winner: draft.lieutenant?.winner ?? null,
    lieutenant_winner_choice: draft.lieutenant?.winnerChoice ?? null,
    lieutenant_other_choice: draft.lieutenant?.otherChoice ?? null,
    sync_pending: true,
  }
}

function draftToSyncPayload(draft: LocalPartieDraft) {
  const isCustom = draft.scenario?.mode === 'other'
  return {
    client_uuid: draft.client_uuid,
    player1: draft.player1,
    player2: draft.adversaire ? '' : draft.player2,
    adversaire: draft.adversaire,
    player1_army_id: draft.player1_army_id,
    player2_army_id: draft.player2_army_id,
    player1_secondary_slugs: draft.player1_secondary_slugs,
    player2_secondary_slugs: draft.player2_secondary_slugs,
    counts_for_elo: draft.counts_for_elo,
    ...(isCustom
      ? {
          scenario_other: draft.scenario?.other,
          scenario_url: draft.scenario?.url,
        }
      : draft.scenario?.id != null
        ? { scenario_id: draft.scenario.id }
        : {}),
    secondary_pool_slugs: draft.secondary_pool_slugs.length
      ? draft.secondary_pool_slugs
      : undefined,
    player1_chosen_secondary: draft.player1_chosen_secondary,
    player2_chosen_secondary: draft.player2_chosen_secondary,
    lieutenant_winner: draft.lieutenant?.winner,
    lieutenant_winner_choice: draft.lieutenant?.winnerChoice,
    lieutenant_other_choice: draft.lieutenant?.otherChoice,
    partie_step: draft.partie_step,
    complete: draft.complete,
  }
}

export async function flushOneLocalPartie(draft: LocalPartieDraft): Promise<MatchRecord> {
  if (draft.server_id && draft.server_id > 0 && !draft.client_uuid) {
    const isCustom = draft.scenario?.mode === 'other'
    await updateMatchProgress(draft.server_id, {
      ...(isCustom
        ? {
            scenario_other: draft.scenario?.other,
            scenario_url: draft.scenario?.url,
          }
        : draft.scenario?.id != null
          ? { scenario_id: draft.scenario.id }
          : {}),
      player1_secondary_slugs: draft.player1_secondary_slugs,
      player2_secondary_slugs: draft.player2_secondary_slugs,
      secondary_pool_slugs: draft.secondary_pool_slugs.length
        ? draft.secondary_pool_slugs
        : undefined,
      player1_chosen_secondary: draft.player1_chosen_secondary,
      player2_chosen_secondary: draft.player2_chosen_secondary,
      lieutenant_winner: draft.lieutenant?.winner,
      lieutenant_winner_choice: draft.lieutenant?.winnerChoice,
      lieutenant_other_choice: draft.lieutenant?.otherChoice,
      partie_step: draft.partie_step,
    })
    if (!draft.complete) {
      return { id: draft.server_id } as MatchRecord
    }
    return completeMatch(draft.server_id, draft.complete)
  }

  return syncPartie(draftToSyncPayload(draft))
}

export async function flushPartieOutbox(): Promise<{
  synced: MatchRecord[]
  errors: string[]
}> {
  const synced: MatchRecord[] = []
  const errors: string[] = []
  for (const draft of listLocalParties()) {
    try {
      const record = await flushOneLocalPartie(draft)
      synced.push(record)
      if (draft.complete || record.status === 'completed') {
        removeLocalPartie(draft.client_uuid)
      } else if (record.id > 0) {
        upsertLocalPartie({ ...draft, server_id: record.id })
      }
    } catch (error) {
      errors.push(error instanceof Error ? error.message : 'Synchro impossible')
    }
  }
  return { synced, errors }
}
