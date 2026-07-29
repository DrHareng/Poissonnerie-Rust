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

/** Vainqueur d'un match d'arbre (objectifs, puis survivants en cas de nul). */
export function bracketMatchWinner(
  match: TournamentMatch,
): 'player1' | 'player2' | null {
  if (match.status !== 'confirmed' || match.is_unplayed) return null
  if (!match.player1 || !match.player2 || !match.outcome) return null

  if (match.outcome === 'player1_win') return 'player1'
  if (match.outcome === 'player2_win') return 'player2'

  if (match.player1_survivors > match.player2_survivors) return 'player1'
  if (match.player2_survivors > match.player1_survivors) return 'player2'
  return null
}

export function matchHasResult(match: TournamentMatch) {
  return (
    match.status === 'confirmed' &&
    !match.is_unplayed &&
    Boolean(match.outcome)
  )
}

export function formatMatchDate(timestamp: number) {
  return new Intl.DateTimeFormat('fr-FR', {
    dateStyle: 'short',
  }).format(new Date(timestamp * 1000))
}

/** Masque les horodatages invalides ou d'époque (ex. id utilisé comme timestamp → 1970). */
export function formatMatchRecordedDate(timestamp: number): string | null {
  if (!timestamp || timestamp < 31_536_000) {
    return null
  }
  const date = new Date(timestamp * 1000)
  if (date.getFullYear() === 1970) {
    return null
  }
  return formatMatchDate(timestamp)
}
