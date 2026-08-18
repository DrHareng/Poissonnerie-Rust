/** Historique sans flag = match classé (comportement d’origine). */
export function matchCountsForElo(countsForElo?: boolean | null): boolean {
  return countsForElo !== false
}

export function casualMatchContextLabel(countsForElo?: boolean | null): string {
  return matchCountsForElo(countsForElo) ? 'Match classé' : 'Match amical'
}
