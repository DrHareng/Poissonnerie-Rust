export type Status = 'preparation' | 'pairing' | 'in_progress' | 'finished'

export interface Sectorial {
  id: number
  parent_id: number
  name: string
  slug: string
  short_name: string
  discontinued: boolean
  faction: string
}

export interface Player {
  id: number
  name: string
  sectorial_id: number
  order: number
  dropped: boolean
  dropped_round?: number | null
  drop_before_pairing_validated?: boolean
}

export interface GameResult {
  p1_op: number
  p1_vp: number
  p2_op: number
  p2_vp: number
  forfeit_player?: number | null
}

export interface Match {
  round: number
  player1: number
  player2?: number | null
  table?: number | null
  result?: GameResult | null
}

export interface Tournament {
  id: string
  name: string
  rounds: number
  tables: number
  round_minutes?: number
  status: Status
  round: number
  players: Player[]
  matches: Match[]
  flagged: [number, number][]
  pending_bye: number | null
  warnings: string[]
}

export interface PlayerCard {
  id: number
  name: string
  sectorial_id: number
  sectorial_name: string
  short_name: string
  dropped: boolean
  can_undo_drop: boolean
}

export interface MatchCard {
  round: number
  table: number | null
  bye: boolean
  player1: PlayerCard
  player2: PlayerCard | null
  p1_op: number | null
  p1_vp: number | null
  p1_tp: number | null
  p2_op: number | null
  p2_vp: number | null
  p2_tp: number | null
  forfeit_player: number | null
  scored: boolean
  flagged: boolean
}

export interface RoundCard {
  round: number
  provisional: boolean
  matches: MatchCard[]
}

export interface StandingRow {
  rank: number
  player_id: number
  name: string
  sectorial_name: string
  short_name: string
  tp: number
  op: number
  vp: number
  rounds_played: number
  op_final: number
  vp_final: number
  wins: number
  draws: number
  losses: number
  byes: number
  dropped: boolean
}

export interface ByeOption {
  player_id: number
  name: string
  short_name: string
  selected: boolean
}

export interface View {
  min_tables: number
  can_start: boolean
  start_reason: string | null
  can_validate_pairing: boolean
  can_validate_round: boolean
  bye_required: boolean
  bye_options: ByeOption[]
  recall: PlayerCard[]
  rounds: RoundCard[]
  standings: StandingRow[]
  warnings: string[]
  final_ranking: boolean
}

export interface EngineOk {
  ok: true
  tournament: Tournament
  view: View
}

export const STATUS_LABEL: Record<Status, string> = {
  preparation: 'Préparation',
  pairing: 'Appariement',
  in_progress: 'Ronde en cours',
  finished: 'Terminé',
}

export function statusLabel(status: Status, round = 0) {
  if (status === 'in_progress') return `Ronde ${round} en cours`
  return STATUS_LABEL[status]
}
