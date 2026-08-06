import { shufflePick } from '@/lib/shufflePick'
import type { SecondaryObjective } from '@/types/elo'

export const COMBAT_ESPRIT_SLUG = 'le-combat-de-lesprit'
export const COMBAT_ESPRIT_POOL_SIZE = 8

export type DraftPlayer = 'A' | 'B'

export type DraftStepKind = 'ban' | 'take'

export type PoolSlotStatus = 'available' | 'banned' | 'taken'

export interface DraftStepDef {
  kind: DraftStepKind
  player: DraftPlayer
  count: number
}

/** Séquence : A bannit, B bannit, A prend 1, B prend 2, A prend 2, B prend 1. */
export const COMBAT_ESPRIT_DRAFT_STEPS: DraftStepDef[] = [
  { kind: 'ban', player: 'A', count: 1 },
  { kind: 'ban', player: 'B', count: 1 },
  { kind: 'take', player: 'A', count: 1 },
  { kind: 'take', player: 'B', count: 2 },
  { kind: 'take', player: 'A', count: 2 },
  { kind: 'take', player: 'B', count: 1 },
]

export interface PoolSlot {
  secondary: SecondaryObjective
  status: PoolSlotStatus
  /** Qui a banni / pris cette carte. */
  by: DraftPlayer | null
}

export interface BannedSecondary {
  secondary: SecondaryObjective
  by: DraftPlayer
}

export interface CombatEspritDraftState {
  /** Toujours 8 emplacements, ordre figé au tirage. */
  slots: PoolSlot[]
  banned: BannedSecondary[]
  playerA: SecondaryObjective[]
  playerB: SecondaryObjective[]
  stepIndex: number
  stepProgress: number
  firstPicker: DraftPlayer
}

type DraftSnapshot = {
  slots: PoolSlot[]
  banned: BannedSecondary[]
  playerA: SecondaryObjective[]
  playerB: SecondaryObjective[]
  stepIndex: number
  stepProgress: number
  firstPicker: DraftPlayer
}

function cloneState(state: CombatEspritDraftState): DraftSnapshot {
  return {
    slots: state.slots.map((slot) => ({ ...slot })),
    banned: state.banned.map((item) => ({ ...item })),
    playerA: [...state.playerA],
    playerB: [...state.playerB],
    stepIndex: state.stepIndex,
    stepProgress: state.stepProgress,
    firstPicker: state.firstPicker,
  }
}

function restoreState(state: CombatEspritDraftState, snapshot: DraftSnapshot) {
  state.slots = snapshot.slots
  state.banned = snapshot.banned
  state.playerA = snapshot.playerA
  state.playerB = snapshot.playerB
  state.stepIndex = snapshot.stepIndex
  state.stepProgress = snapshot.stepProgress
  state.firstPicker = snapshot.firstPicker
}

export function createCombatEspritDraft(
  secondaries: SecondaryObjective[],
  firstPicker: DraftPlayer = 'A',
): CombatEspritDraftState {
  const pool = shufflePick(secondaries, Math.min(COMBAT_ESPRIT_POOL_SIZE, secondaries.length))
  return {
    slots: pool.map((secondary) => ({
      secondary,
      status: 'available',
      by: null,
    })),
    banned: [],
    playerA: [],
    playerB: [],
    stepIndex: 0,
    stepProgress: 0,
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
  card: SecondaryObjective,
): void {
  if (player === 'A') {
    state.playerA.push(card)
  } else {
    state.playerB.push(card)
  }
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
  return currentDraftStep(state)?.player ?? null
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

/** Numéros d’étape (1–6) pour la case ban + les 3 picks d’un joueur A/B. */
export function draftSlotStepNumbers(player: DraftPlayer): {
  ban: number
  picks: [number, number, number]
} {
  let ban = 0
  const picks: number[] = []
  COMBAT_ESPRIT_DRAFT_STEPS.forEach((step, index) => {
    if (step.player !== player) return
    const n = index + 1
    if (step.kind === 'ban') {
      ban = n
      return
    }
    for (let i = 0; i < step.count; i += 1) {
      picks.push(n)
    }
  })
  return {
    ban,
    picks: [picks[0] ?? 0, picks[1] ?? 0, picks[2] ?? 0],
  }
}

/** Badges à afficher : un seul numéro à cheval si deux picks partagent l’étape. */
export function draftStepBadges(player: DraftPlayer): Array<{
  number: number
  /** Colonne grille 1–4 (ban = 1, picks = 2–4). */
  columnStart: number
  span: number
}> {
  const { ban, picks } = draftSlotStepNumbers(player)
  const badges: Array<{ number: number; columnStart: number; span: number }> = [
    { number: ban, columnStart: 1, span: 1 },
  ]
  let i = 0
  while (i < picks.length) {
    const number = picks[i]!
    let span = 1
    while (i + span < picks.length && picks[i + span] === number) {
      span += 1
    }
    badges.push({ number, columnStart: 2 + i, span })
    i += span
  }
  return badges
}

export function draftPlayerIsSlot1(
  player: DraftPlayer,
  firstPicker: DraftPlayer,
): boolean {
  return (
    (player === 'A' && firstPicker === 'A') ||
    (player === 'B' && firstPicker === 'B')
  )
}

const historyStack = new WeakMap<CombatEspritDraftState, DraftSnapshot[]>()

function getHistory(state: CombatEspritDraftState): DraftSnapshot[] {
  let history = historyStack.get(state)
  if (!history) {
    history = []
    historyStack.set(state, history)
  }
  return history
}

export function canUndoDraft(state: CombatEspritDraftState): boolean {
  return (historyStack.get(state)?.length ?? 0) > 0
}

export function undoDraft(state: CombatEspritDraftState): boolean {
  const history = historyStack.get(state)
  const previous = history?.pop()
  if (!previous) return false
  restoreState(state, previous)
  return true
}

export function chooseCard(
  state: CombatEspritDraftState,
  slug: string,
): boolean {
  const step = currentDraftStep(state)
  if (!step) return false

  const slot = state.slots.find(
    (item) => item.status === 'available' && item.secondary.slug === slug,
  )
  if (!slot) return false

  getHistory(state).push(cloneState(state))

  if (step.kind === 'ban') {
    slot.status = 'banned'
    slot.by = step.player
    state.banned.push({ secondary: slot.secondary, by: step.player })
  } else {
    slot.status = 'taken'
    slot.by = step.player
    assignToPlayer(state, step.player, slot.secondary)
  }

  state.stepProgress += 1
  if (state.stepProgress >= step.count) {
    state.stepIndex += 1
    state.stepProgress = 0
  }

  return true
}

export function availableSlots(state: CombatEspritDraftState): PoolSlot[] {
  return state.slots.filter((slot) => slot.status === 'available')
}

export function mapDraftToPlayers(
  state: CombatEspritDraftState,
  firstPicker: DraftPlayer,
): {
  player1: string[]
  player2: string[]
  bannedPlayer1: string | null
  bannedPlayer2: string | null
  pool: string[]
} {
  const aSlugs = state.playerA.map((item) => item.slug)
  const bSlugs = state.playerB.map((item) => item.slug)
  const banA = state.banned.find((item) => item.by === 'A')?.secondary.slug ?? null
  const banB = state.banned.find((item) => item.by === 'B')?.secondary.slug ?? null
  const pool = state.slots.map((slot) => slot.secondary.slug)

  if (firstPicker === 'A') {
    return {
      player1: aSlugs,
      player2: bSlugs,
      bannedPlayer1: banA,
      bannedPlayer2: banB,
      pool,
    }
  }
  return {
    player1: bSlugs,
    player2: aSlugs,
    bannedPlayer1: banB,
    bannedPlayer2: banA,
    pool,
  }
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

export function bannedForDisplay(
  state: CombatEspritDraftState,
  firstPicker: DraftPlayer,
): { left: SecondaryObjective[]; right: SecondaryObjective[] } {
  const left: SecondaryObjective[] = []
  const right: SecondaryObjective[] = []
  for (const item of state.banned) {
    if (draftPlayerIsSlot1(item.by, firstPicker)) {
      left.push(item.secondary)
    } else {
      right.push(item.secondary)
    }
  }
  return { left, right }
}

export type PoolStripSlotView = {
  slug: string
  status: PoolSlotStatus
  owner: 'player1' | 'player2' | null
}

/** Reconstruit la ligne de 8 pour la page match. */
export function buildPoolStripView(args: {
  pool: string[]
  player1Taken: string[]
  player2Taken: string[]
  bannedPlayer1?: string | null
  bannedPlayer2?: string | null
}): PoolStripSlotView[] {
  const p1 = new Set(args.player1Taken)
  const p2 = new Set(args.player2Taken)
  const banned = new Set(
    [args.bannedPlayer1, args.bannedPlayer2].filter(
      (slug): slug is string => Boolean(slug),
    ),
  )

  return args.pool.map((slug) => {
    if (banned.has(slug)) {
      return { slug, status: 'banned' as const, owner: null }
    }
    if (p1.has(slug)) {
      return { slug, status: 'taken' as const, owner: 'player1' as const }
    }
    if (p2.has(slug)) {
      return { slug, status: 'taken' as const, owner: 'player2' as const }
    }
    return { slug, status: 'available' as const, owner: null }
  })
}
