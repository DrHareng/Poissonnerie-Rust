import type { MatchRecord } from '@/types/elo'

function samePlayer(a: string | null | undefined, b: string | null | undefined) {
  if (!a || !b) return false
  return a.localeCompare(b, undefined, { sensitivity: 'accent' }) === 0
}

export function isMatchPlayer(
  match: Pick<MatchRecord, 'player1' | 'player2'>,
  playerName?: string | null,
) {
  if (!playerName) return false
  return samePlayer(match.player1, playerName) || samePlayer(match.player2, playerName)
}

/** Assistant de saisie : joueur de la partie, ou admin en mode édition. */
export function canResumePartie(
  match: Pick<MatchRecord, 'player1' | 'player2' | 'status'>,
  options: { playerName?: string | null; isEditMode: boolean },
) {
  if (match.status !== 'in_progress') return false
  if (options.isEditMode) return true
  return isMatchPlayer(match, options.playerName)
}
