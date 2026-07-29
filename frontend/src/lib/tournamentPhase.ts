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
