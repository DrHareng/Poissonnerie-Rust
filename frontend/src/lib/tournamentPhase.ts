import type { TournamentPhase } from '@/types/elo'

export const phaseLabels: Record<TournamentPhase, string> = {
  pool: 'Poule',
  round_of_16: '1/8 de finale',
  quarter: '1/4 de final',
  semi: 'Demi-finale',
  final: 'Finale',
}

export function phaseLabel(phase?: string | null) {
  if (!phase) return null
  return phaseLabels[phase as TournamentPhase] ?? phase
}

/** Poule nommée (Poule C) si connue, sinon le libellé de phase. */
export function tournamentPhaseDisplay(match: {
  tournament_phase?: string | null
  tournament_pool_name?: string | null
}): string | null {
  const poolName = match.tournament_pool_name?.trim()
  if (poolName) return poolName
  return phaseLabel(match.tournament_phase)
}
