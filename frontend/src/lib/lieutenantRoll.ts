export type LieutenantWinner = 'player1' | 'player2'

export type LieutenantWinnerChoice =
  | 'initiative-first'
  | 'initiative-second'
  | 'deployment-first'
  | 'deployment-second'

export type LieutenantOtherChoice = LieutenantWinnerChoice

export interface LieutenantRollChoice {
  value: LieutenantWinnerChoice
  label: string
}

export interface PartieLieutenant {
  winner: LieutenantWinner
  winnerChoice: LieutenantWinnerChoice
  otherChoice: LieutenantOtherChoice
}

export const WINNER_CHOICES: LieutenantRollChoice[] = [
  { value: 'initiative-first', label: "prend l'initiative : jouera en premier" },
  { value: 'initiative-second', label: "prend l'initiative : jouera en second" },
  {
    value: 'deployment-first',
    label: 'prend le déploiement : se déploiera en premier',
  },
  {
    value: 'deployment-second',
    label: 'prend le déploiement : se déploiera en second',
  },
]

export function otherPlayerChoices(
  winnerChoice: LieutenantWinnerChoice,
): LieutenantRollChoice[] {
  if (winnerChoice.startsWith('initiative')) {
    return WINNER_CHOICES.filter((choice) => choice.value.startsWith('deployment'))
  }
  return WINNER_CHOICES.filter((choice) => choice.value.startsWith('initiative'))
}

export function choiceLabel(value: string): string {
  return WINNER_CHOICES.find((choice) => choice.value === value)?.label ?? value
}
