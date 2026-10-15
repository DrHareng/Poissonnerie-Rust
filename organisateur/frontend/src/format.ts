import type { MatchCard } from './types'

export type ScoreTone = 'win' | 'loss' | 'draw'

export function scoreTone(game: MatchCard, side: 1 | 2): ScoreTone {
  const playerId = side === 1 ? game.player1.id : game.player2?.id
  if (game.forfeit_player != null && playerId != null) {
    return game.forfeit_player === playerId ? 'loss' : 'win'
  }
  const mine = side === 1 ? game.p1_op : game.p2_op
  const other = side === 1 ? game.p2_op : game.p1_op
  if (mine == null || other == null || mine === other) return 'draw'
  return mine > other ? 'win' : 'loss'
}

export function armyLogoUrl(slug: string) {
  return `/factions/${slug}.svg`
}

export function formatPoints(value: number) {
  return new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 2 }).format(value)
}
