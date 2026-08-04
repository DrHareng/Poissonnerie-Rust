import { shufflePick } from '@/lib/shufflePick'
import type { SecondaryObjective } from '@/types/elo'

export const COMBAT_ESPRIT_SLUG = 'le-combat-de-lesprit'

export type DraftPlayer = 'A' | 'B'

export type DraftStepKind = 'pick' | 'take'

export interface DraftStepDef {
  kind: DraftStepKind
  player: DraftPlayer
  count: number
  label: string
}

export const COMBAT_ESPRIT_DRAFT_STEPS: DraftStepDef[] = [
  { kind: 'pick', player: 'A', count: 1, label: 'Joueur A choisit 1 objectif' },
  { kind: 'pick', player: 'B', count: 1, label: 'Joueur B choisit 1 objectif' },
  { kind: 'take', player: 'A', count: 1, label: 'Joueur A prend 1 objectif' },
  { kind: 'take', player: 'B', count: 2, label: 'Joueur B prend 2 objectifs' },
  { kind: 'take', player: 'A', count: 2, label: 'Joueur A prend 2 objectifs' },
  { kind: 'take', player: 'B', count: 1, label: 'Joueur B prend 1 objectif' },
]

export interface CombatEspritDraftState {
  deck: SecondaryObjective[]
  playerA: SecondaryObjective[]
  playerB: SecondaryObjective[]
  stepIndex: number
  firstPicker: DraftPlayer
}

export function createCombatEspritDraft(
  secondaries: SecondaryObjective[],
  firstPicker: DraftPlayer = 'A',
): CombatEspritDraftState {
  return {
    deck: shufflePick(secondaries, secondaries.length),
    playerA: [],
    playerB: [],
    stepIndex: 0,
    firstPicker,
  }
}

function playerHand(
  state: CombatEspritDraftState,
  player: DraftPlayer,
): SecondaryObjective[] {
  return player === 'A' ? state.playerA : state.playerB
}

function assignToPlayer(
  state: CombatEspritDraftState,
  player: DraftPlayer,
  cards: SecondaryObjective[],
): void {
  if (player === 'A') {
    state.playerA.push(...cards)
  } else {
    state.playerB.push(...cards)
  }
}

function removeFromDeck(
  state: CombatEspritDraftState,
  slugs: Set<string>,
): void {
  state.deck = state.deck.filter((card) => !slugs.has(card.slug))
}

export function currentDraftStep(
  state: CombatEspritDraftState,
): DraftStepDef | null {
  return COMBAT_ESPRIT_DRAFT_STEPS[state.stepIndex] ?? null
}

export function isDraftComplete(state: CombatEspritDraftState): boolean {
  return state.stepIndex >= COMBAT_ESPRIT_DRAFT_STEPS.length
}

export function activeDraftPlayer(
  state: CombatEspritDraftState,
): DraftPlayer | null {
  const step = currentDraftStep(state)
  if (!step) return null
  return step.player
}

/** Pick a card during an interactive step. Returns false if not a pick step. */
export function pickCard(
  state: CombatEspritDraftState,
  slug: string,
): boolean {
  const step = currentDraftStep(state)
  if (!step || step.kind !== 'pick') return false

  const card = state.deck.find((item) => item.slug === slug)
  if (!card) return false

  assignToPlayer(state, step.player, [card])
  removeFromDeck(state, new Set([slug]))
  advanceAfterStep(state)
  return true
}

function advanceAfterStep(state: CombatEspritDraftState): void {
  state.stepIndex++
  runAutoSteps(state)
}

/** Run consecutive automatic "take" steps until the next pick or end. */
export function runAutoSteps(state: CombatEspritDraftState): void {
  while (true) {
    const step = currentDraftStep(state)
    if (!step || step.kind !== 'take') break

    const taken = shufflePick(state.deck, step.count)
    if (taken.length === 0) {
      state.stepIndex++
      continue
    }

    assignToPlayer(state, step.player, taken)
    removeFromDeck(state, new Set(taken.map((item) => item.slug)))
    state.stepIndex++
  }
}

export function draftPlayerLabel(
  player: DraftPlayer,
  firstPicker: DraftPlayer,
  player1Name: string,
  player2Name: string,
): string {
  const aName = firstPicker === 'A' ? player1Name : player2Name
  const bName = firstPicker === 'A' ? player2Name : player1Name
  return player === 'A' ? aName : bName
}

export function mapDraftToPlayers(
  state: CombatEspritDraftState,
  firstPicker: DraftPlayer,
): { player1: string[]; player2: string[] } {
  const aSlugs = state.playerA.map((item) => item.slug)
  const bSlugs = state.playerB.map((item) => item.slug)
  if (firstPicker === 'A') {
    return { player1: aSlugs, player2: bSlugs }
  }
  return { player1: bSlugs, player2: aSlugs }
}

export function handForPlayer(
  state: CombatEspritDraftState,
  slot: 1 | 2,
  firstPicker: DraftPlayer,
): SecondaryObjective[] {
  const draftPlayer: DraftPlayer =
    (slot === 1 && firstPicker === 'A') || (slot === 2 && firstPicker === 'B')
      ? 'A'
      : 'B'
  return playerHand(state, draftPlayer)
}
