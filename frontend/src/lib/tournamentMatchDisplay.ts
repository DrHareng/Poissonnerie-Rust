import type { TournamentMatch } from '@/types/elo'

export function matchPlayerScores(
  match: TournamentMatch,
  slot: 'player1' | 'player2',
) {
  if (slot === 'player1') {
    return {
      pt: match.player1_tournament_points,
      po: match.player1_objectives,
      ps: match.player1_survivors,
    }
  }
  return {
    pt: match.player2_tournament_points,
    po: match.player2_objectives,
    ps: match.player2_survivors,
  }
}

export function formatMatchDate(timestamp: number) {
  return new Intl.DateTimeFormat('fr-FR', {
    dateStyle: 'short',
  }).format(new Date(timestamp * 1000))
}
