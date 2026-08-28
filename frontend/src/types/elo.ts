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

export type MatchStatus = 'in_progress' | 'completed'

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

export type ReportStatus = 'draft' | 'published'

export interface MatchReport {
  id: number
  body_md: string
  status?: ReportStatus
  published_at?: number | null
  created_at: number
  updated_at: number
}

export interface RecentMatchReport {
  match_id: number
  report_id: number
  author_name: string
  author_display_name: string
  author_slot: 'player1' | 'player2'
  opponent_name: string
  opponent_display_name: string
  author_army_id?: number | null
  opponent_army_id?: number | null
  scenario_name?: string | null
  tournament_id?: number | null
  tournament_phase?: string | null
  tournament_name?: string | null
  counts_for_elo?: boolean
  excerpt: string
  published_at: number
  updated_at: number
}

export interface ReportTemplate {
  id: number
  name: string
  body_md: string
  created_at: number
  updated_at: number
}

export interface MatchRecord extends RatingUpdate, MatchScores {
  id: number
  player1: string
  player2: string
  player1_display_name?: string
  player2_display_name?: string
  status?: MatchStatus
  outcome?: MatchOutcome | null
  player1_army_id?: number | null
  player2_army_id?: number | null
  scenario_id?: number | null
  scenario_other?: string | null
  scenario_url?: string | null
  scenario_name?: string | null
  tournament_id?: number | null
  tournament_phase?: string | null
  tournament_name?: string | null
  player1_report?: MatchReport | null
  player2_report?: MatchReport | null
  player1_army_list_code?: string | null
  player2_army_list_code?: string | null
  player1_secondary_slugs?: string[] | null
  player2_secondary_slugs?: string[] | null
  secondary_pool_slugs?: string[] | null
  player1_chosen_secondary?: string | null
  player2_chosen_secondary?: string | null
  lieutenant_winner?: string | null
  lieutenant_winner_choice?: string | null
  lieutenant_other_choice?: string | null
  partie_step?: string | null
  created_by?: string | null
  /** false = match amical (pas d'impact ELO). Défaut true pour l'historique. */
  counts_for_elo?: boolean
  recorded_at: number
}

export interface Scenario {
  id: number
  name: string
  usage_count: number
  slug?: string | null
  map_filename?: string | null
  pack_id?: number | null
}

export interface ScenarioPack {
  id: number
  slug: string
  name: string
  version?: string | null
  preamble_md: string
}

export interface ScenarioSummary {
  id: number
  slug: string
  name: string
  flavor_text?: string | null
  map_filename?: string | null
  sort_order: number
}

export interface ScenarioPackPage {
  pack: ScenarioPack
  scenarios: ScenarioSummary[]
}

export interface SecondaryObjective {
  id: number
  slug: string
  name: string
  body_md: string
}

export interface CommonRule {
  id: number
  slug: string
  name: string
  body_md: string
}

export interface ScenarioDetail {
  id: number
  slug: string
  name: string
  flavor_text?: string | null
  map_filename?: string | null
  end_condition_md?: string | null
  objectives_md?: string | null
  deployment_notes_md?: string | null
  exclusion_zones_md?: string | null
  elements_md?: string | null
  special_rules_md?: string | null
  sort_order: number
  exclusion_rule?: CommonRule | null
  common_rules: CommonRule[]
}

export const DEFAULT_SCENARIO_PACK_SLUG = 'poissonnerie-v2'

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
  description: string
  status: TournamentStatus
  pool_count: number
  bracket_format: BracketFormat
  created_at: number
  started_at?: number | null
  pools_finalized_at?: number | null
  completed_at?: number | null
  list_validator_user_id?: number | null
  list_validator_display_name?: string | null
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
  army_list_1?: string | null
  army_list_2?: string | null
  bracket_list_1?: string | null
  bracket_list_2?: string | null
  has_army_lists?: boolean
  has_bracket_lists?: boolean
  has_army_list_2?: boolean
  has_bracket_list_2?: boolean
}

export interface TournamentScenarioSlot {
  kind: string
  slot: string
  scenario_id: number
  scenario_name: string
  scenario_slug?: string
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
  scenario_other?: string | null
  scenario_name?: string | null
  player1_army_id?: number | null
  player2_army_id?: number | null
  player1_army_list_code?: string | null
  player2_army_list_code?: string | null
  played_at?: number | null
  elo_match_id?: number | null
}

export interface TournamentDetail extends Tournament {
  registrations: TournamentRegistration[]
  players: unknown[]
  pools: Pool[]
  matches: TournamentMatch[]
  registered_count: number
  waitlist_count: number
  display_status: string
  top_four?: TournamentTopFourEntry[]
  pool_scenarios?: TournamentScenarioSlot[]
  bracket_scenario_pool?: TournamentScenarioSlot[]
  bracket_scenarios?: TournamentScenarioSlot[]
}

export interface TournamentTopFourEntry {
  rank: number
  player_name: string
  player_display_name?: string | null
  army_id?: number | null
}

export interface TournamentRegistrationPreview {
  player_name: string
  player_display_name?: string | null
  status: RegistrationStatus
  has_army_lists?: boolean
}

export interface TournamentListEntry extends Tournament {
  registered_count: number
  waitlist_count: number
  display_status: string
  top_four?: TournamentTopFourEntry[]
  bracket_matches?: TournamentMatch[]
  pool_scenarios?: TournamentScenarioSlot[]
  registrations?: TournamentRegistrationPreview[]
}

export interface PlayerTournamentResult {
  tournament_id: number
  tournament_name: string
  placement_label: string
  final_placement?: number | null
  completed_at?: number | null
  army_id?: number | null
}

export interface PaginatedMatches {
  items: MatchRecord[]
  total: number
  limit: number
  offset: number
}

export interface PaginatedReports {
  items: RecentMatchReport[]
  total: number
  limit: number
  offset: number
}

export interface ApiError {
  error: string
}
