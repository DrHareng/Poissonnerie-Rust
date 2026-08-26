import type { MatchRecord } from '@/types/elo'

export type BuiltinReportTemplateId = 'classique' | 'court'

export interface BuiltinReportTemplate {
  id: BuiltinReportTemplateId
  name: string
  body_md: string
}

export function samePlayerName(a: string, b: string): boolean {
  return a.localeCompare(b, undefined, { sensitivity: 'accent' }) === 0
}

function turnBlocks(turns: number): string {
  return Array.from({ length: turns }, (_, index) => {
    const turn = index + 1
    return `## T${turn} — [joueur1]\n\n\n## T${turn} — [joueur2]\n`
  }).join('\n')
}

export const BUILTIN_REPORT_TEMPLATES: BuiltinReportTemplate[] = [
  {
    id: 'classique',
    name: 'Classique (6 tours)',
    body_md: [
      '## Listes',
      '',
      '### [joueur1]',
      '',
      '',
      '### [joueur2]',
      '',
      '',
      '## Déploiement',
      '',
      '[deploy1] se déploie en premier.',
      '',
      '[deploy2] se déploie en second.',
      '',
      '',
      turnBlocks(6),
      '## Conclusion',
      '',
      '',
    ].join('\n'),
  },
  {
    id: 'court',
    name: 'Court',
    body_md: [
      '## Listes',
      '',
      '### [moi]',
      '',
      '',
      '### [adversaire]',
      '',
      '',
      '## Déploiement',
      '',
      '[deploy1] se déploie en premier.',
      '',
      '',
      '## Résumé',
      '',
      '',
      '## Conclusion',
      '',
      '',
    ].join('\n'),
  },
]

type Slot = 'player1' | 'player2'

function otherSlot(slot: Slot): Slot {
  return slot === 'player1' ? 'player2' : 'player1'
}

function winnerSlot(match: MatchRecord): Slot {
  return match.lieutenant_winner === 'player2' ? 'player2' : 'player1'
}

function choiceKind(value: string | null | undefined): 'initiative' | 'deployment' | null {
  if (!value) return null
  if (value.startsWith('initiative')) return 'initiative'
  if (value.startsWith('deployment')) return 'deployment'
  return null
}

function isFirst(choice: string | null | undefined): boolean {
  return Boolean(choice && choice.endsWith('-first'))
}

function orderFor(
  match: MatchRecord,
  kind: 'initiative' | 'deployment',
): [Slot, Slot] {
  const winner = winnerSlot(match)
  const other = otherSlot(winner)
  const winnerChoice = match.lieutenant_winner_choice
  const otherChoice = match.lieutenant_other_choice

  if (choiceKind(winnerChoice) === kind) {
    return isFirst(winnerChoice) ? [winner, other] : [other, winner]
  }
  if (choiceKind(otherChoice) === kind) {
    return isFirst(otherChoice) ? [other, winner] : [winner, other]
  }
  return ['player1', 'player2']
}

function displayName(match: MatchRecord, slot: Slot): string {
  if (slot === 'player1') {
    return match.player1_display_name ?? match.player1
  }
  return match.player2_display_name ?? match.player2
}

export function applyReportTemplate(
  source: string,
  match: MatchRecord,
  authorName: string,
): string {
  const [first, second] = orderFor(match, 'initiative')
  const [deployFirst, deploySecond] = orderFor(match, 'deployment')
  const authorSlot: Slot = samePlayerName(authorName, match.player1)
    ? 'player1'
    : 'player2'
  const replacements: Array<[string, string]> = [
    ['[joueur1]', displayName(match, first)],
    ['[joueur2]', displayName(match, second)],
    ['[deploy1]', displayName(match, deployFirst)],
    ['[deploy2]', displayName(match, deploySecond)],
    ['[moi]', displayName(match, authorSlot)],
    ['[adversaire]', displayName(match, otherSlot(authorSlot))],
  ]
  let result = source
  for (const [token, value] of replacements) {
    result = result.split(token).join(value)
  }
  return result
}
