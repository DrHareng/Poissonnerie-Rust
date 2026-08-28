import type { RegistrationStatus, TournamentTopFourEntry } from '@/types/elo'

export interface RegistrationSortInput {
  player_name: string
  player_display_name?: string | null
  status: RegistrationStatus
  has_army_lists?: boolean
}

const registrationStatusLabels: Record<string, string> = {
  approved: 'Validé',
  waitlisted: "Liste d'attente",
  rejected: 'Refusé',
}

export function registrationStatusLabel(reg: {
  status: RegistrationStatus
  has_army_lists?: boolean
}): string {
  const waitingLists =
    !reg.has_army_lists
    && (reg.status === 'pending' || reg.status === 'waitlisted')
  if (waitingLists) return 'En attente des listes'
  if (reg.status === 'pending') return 'En attente de validation'
  return registrationStatusLabels[reg.status] ?? reg.status
}

/** Validé (0) puis le reste ; « en attente des listes » en dernier (2). */
export function registrationSortTier(reg: RegistrationSortInput): number {
  if (reg.status === 'approved') return 0
  if (
    !reg.has_army_lists
    && (reg.status === 'pending' || reg.status === 'waitlisted')
  ) {
    return 2
  }
  return 1
}

export function compareRegistrationsForDisplay(
  a: RegistrationSortInput,
  b: RegistrationSortInput,
): number {
  const tierDiff = registrationSortTier(a) - registrationSortTier(b)
  if (tierDiff !== 0) return tierDiff
  const nameA = (a.player_display_name ?? a.player_name).toLocaleLowerCase('fr')
  const nameB = (b.player_display_name ?? b.player_name).toLocaleLowerCase('fr')
  return nameA.localeCompare(nameB, 'fr')
}

export function sortRegistrationsForDisplay<T extends RegistrationSortInput>(
  registrations: T[],
): T[] {
  return [...registrations].sort(compareRegistrationsForDisplay)
}

export function isTournamentRegistrationPhase(status: string): boolean {
  return status === 'registration_open' || status === 'registration_closed'
}

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
