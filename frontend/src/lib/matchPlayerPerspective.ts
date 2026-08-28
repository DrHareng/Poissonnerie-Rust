import type { MatchOutcome, MatchRecord } from '@/types/elo'

function normalize(name: string) {
  return name.trim().toLowerCase()
}

export function isSamePlayer(a: string, b: string) {
  return normalize(a) === normalize(b)
}

function flipOutcome(outcome: MatchOutcome | null | undefined): MatchOutcome {
  if (!outcome || outcome === 'draw') return 'draw'
  if (outcome === 'player1_win') return 'player2_win'
  return 'player1_win'
}

/** Place le joueur donné en position 1 (scores, ELO, armées). */
export function normalizeMatchForPlayer(
  match: MatchRecord,
  playerName: string,
): MatchRecord {
  if (isSamePlayer(match.player1, playerName)) {
    return match
  }

  return {
    ...match,
    player1: match.player2,
    player2: match.player1,
    player1_display_name: match.player2_display_name,
    player2_display_name: match.player1_display_name,
    player1_old: match.player2_old,
    player1_new: match.player2_new,
    player2_old: match.player1_old,
    player2_new: match.player1_new,
    player1_objectives: match.player2_objectives,
    player1_survivors: match.player2_survivors,
    player2_objectives: match.player1_objectives,
    player2_survivors: match.player1_survivors,
    player1_army_id: match.player2_army_id,
    player2_army_id: match.player1_army_id,
    player1_army_list_code: match.player2_army_list_code,
    player2_army_list_code: match.player1_army_list_code,
    outcome: flipOutcome(match.outcome),
  }
}

export function playerMatchEloDelta(match: MatchRecord): number | null {
  if (match.player1_old == null || match.player1_new == null) return null
  return match.player1_new - match.player1_old
}
