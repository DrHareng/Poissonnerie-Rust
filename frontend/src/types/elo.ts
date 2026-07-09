export interface Army {
  id: number
  parent_id: number
  name: string
  slug: string
  logo_url: string
  discontinued: boolean
}

export interface PlayerArmyUsage {
  army_id: number
  matches: number
  last_played_at: number
}

export interface Player {
  name: string
  rating: number
  wins: number
  draws: number
  losses: number
  discord_username?: string | null
}

export interface User {
  id: number
  discord_id: string
  username: string
  display_name: string
  avatar_url: string
  effective_display_name: string
  effective_avatar_url: string
  local_display_name?: string | null
  local_avatar_url?: string | null
  is_admin: boolean
  created_at: number
  last_login_at: number
}

export interface PlayerProfile extends Player {
  display_name: string
  avatar_url?: string | null
  profile_display_name?: string | null
  discord_display_name?: string | null
  is_own_profile: boolean
}

export interface AuthUser {
  user: User
  player: Player | null
}

export interface RankedPlayer extends Player {
  rank: number
  display_name: string
  top_armies: PlayerArmyUsage[]
  star_count?: number
}

export interface RankedArmy {
  rank: number
  army_id: number
  wins: number
  draws: number
  losses: number
  win_rate: number
}

export type MatchOutcome = 'player1_win' | 'player2_win' | 'draw'

export interface RatingUpdate {
  player1_old: number
  player1_new: number
  player2_old: number
  player2_new: number
}

export interface MatchScores {
  player1_objectives: number
  player1_survivors: number
  player2_objectives: number
  player2_survivors: number
}

export interface MatchRecord extends RatingUpdate, MatchScores {
  id: number
  player1: string
  player2: string
  player1_display_name?: string
  player2_display_name?: string
  outcome: MatchOutcome
  player1_army_id?: number | null
  player2_army_id?: number | null
  scenario_id?: number | null
  scenario_name?: string | null
  recorded_at: number
}

export interface Scenario {
  id: number
  name: string
  usage_count: number
}

export type TournamentStatus =
  | 'draft'
  | 'registration_open'
  | 'registration_closed'
  | 'started'
  | 'completed'

export type RegistrationStatus = 'pending' | 'approved' | 'waitlisted' | 'rejected'

export type BracketFormat = 'quarters_direct' | 'round_of_16' | 'round_of_16_full'

export type TournamentPhase = 'pool' | 'round_of_16' | 'quarter' | 'semi' | 'final'

export type TournamentMatchStatus = 'scheduled' | 'submitted' | 'confirmed'

export interface Tournament {
  id: number
  name: string
  status: TournamentStatus
  pool_count: number
  bracket_format: BracketFormat
  created_at: number
  started_at?: number | null
  pools_finalized_at?: number | null
  completed_at?: number | null
}

export interface TournamentRegistration {
  id: number
  tournament_id: number
  player_name: string
  player_display_name?: string | null
  user_id?: number | null
  status: RegistrationStatus
  waitlist_position?: number | null
  requested_at: number
  reviewed_at?: number | null
  reviewed_by?: number | null
  army_id?: number | null
}

export interface PoolPlayer {
  player_name: string
  player_display_name?: string | null
  army_id?: number | null
  seed: number
  points: number
  objectives: number
  survivors: number
  wins: number
  draws: number
  losses: number
}

export interface Pool {
  id: number
  tournament_id: number
  name: string
  position: number
  players: PoolPlayer[]
}

export interface TournamentMatch {
  id: number
  tournament_id: number
  phase: TournamentPhase
  pool_id?: number | null
  bracket_slot?: number | null
  player1?: string | null
  player2?: string | null
  player1_display_name?: string | null
  player2_display_name?: string | null
  player1_objectives: number
  player2_objectives: number
  player1_survivors: number
  player2_survivors: number
  player1_tournament_points: number
  player2_tournament_points: number
  outcome?: MatchOutcome | null
  is_forfeit: boolean
  is_unplayed?: boolean
  forfeit_player?: string | null
  forfeit_player_display_name?: string | null
  status: TournamentMatchStatus
  scenario_id?: number | null
  scenario_name?: string | null
  player1_army_id?: number | null
  player2_army_id?: number | null
  played_at?: number | null
}

export interface TournamentDetail extends Tournament {
  registrations: TournamentRegistration[]
  players: unknown[]
  pools: Pool[]
  matches: TournamentMatch[]
  approved_count: number
  waitlist_count: number
  display_status: string
  top_four?: TournamentTopFourEntry[]
}

export interface TournamentTopFourEntry {
  rank: number
  player_name: string
  player_display_name?: string | null
}

export interface TournamentListEntry extends Tournament {
  approved_count: number
  waitlist_count: number
  display_status: string
  top_four?: TournamentTopFourEntry[]
}

export interface PlayerTournamentResult {
  tournament_id: number
  tournament_name: string
  placement_label: string
  final_placement?: number | null
  completed_at?: number | null
}

export interface ApiError {
  error: string
}
