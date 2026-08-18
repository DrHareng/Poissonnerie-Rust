import type { TournamentTopFourEntry } from '@/types/elo'

export function formatRegistrationSummary(
  registered: number,
  waitlist: number,
  capacity = 24,
) {
  const base = `${registered}/${capacity} inscrit${registered > 1 ? 's' : ''}`
  if (waitlist > 0) {
    return `${base} (+${waitlist} en liste d'attente)`
  }
  return base
}

export function tournamentRegistrationCapacity(poolCount: number) {
  return poolCount >= 8 ? 48 : 24
}

export interface TopFourDisplayRow {
  label: string
  entries: TournamentTopFourEntry[]
}

/** Regroupe le top 4 : 1er, 2e, puis 3-4 (demi-finalistes, sans petite finale). */
export function topFourDisplayRows(
  entries: TournamentTopFourEntry[],
): TopFourDisplayRow[] {
  if (entries.length === 0) return []

  const byRank = new Map(entries.map((entry) => [entry.rank, entry]))
  const rows: TopFourDisplayRow[] = []

  const first = byRank.get(1)
  if (first) rows.push({ label: '1', entries: [first] })

  const second = byRank.get(2)
  if (second) rows.push({ label: '2', entries: [second] })

  const semiFinalists = entries.filter((entry) => entry.rank === 3 || entry.rank === 4)
  if (semiFinalists.length > 0) {
    rows.push({ label: '3-4', entries: semiFinalists })
  }

  return rows
}
