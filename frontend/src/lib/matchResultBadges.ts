/** Libellé affiché dans un badge de score (objectifs - survivants). */
export function matchScoreLabel(objectives: number, survivors: number) {
  return `${objectives} - ${survivors}`
}

type ScoreBadgeMatch = {
  player1_objectives: number
  player2_objectives: number
  player1_survivors: number
  player2_survivors: number
  outcome?: string | null
  status?: string
  is_unplayed?: boolean
  is_forfeit?: boolean
  awaiting_confirmation?: boolean
  elo_match_id?: number | null
}

/**
 * Largeur mini (en `ch`) pour aligner les badges score d'une colonne.
 * Pour un libellé « En cours » / « Non joué » (badge unique), on prend la moitié
 * car la pastille occupe la largeur des deux badges.
 */
export function scoreBadgeMinCh(
  matches: ScoreBadgeMatch[],
  pendingLabel = 'En cours',
): number {
  let max = 0
  for (const match of matches) {
    const pending =
      match.is_unplayed
      || (match.is_forfeit && !match.outcome)
      || match.status === 'in_progress'
      || match.status === 'submitted'
      || match.awaiting_confirmation
      || (match.status === 'scheduled' && Boolean(match.elo_match_id))
      || !match.outcome
    if (pending) {
      const label = match.is_unplayed
        ? 'Non joué'
        : match.is_forfeit && !match.outcome
          ? 'Forfait'
          : match.awaiting_confirmation || match.status === 'submitted'
            ? 'À confirmer'
            : pendingLabel
      max = Math.max(max, Math.ceil(label.length / 2))
      continue
    }
    max = Math.max(
      max,
      matchScoreLabel(match.player1_objectives, match.player1_survivors).length,
      matchScoreLabel(match.player2_objectives, match.player2_survivors).length,
    )
  }
  return Math.max(max, 1)
}
