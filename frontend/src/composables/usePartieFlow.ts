import { computed, ref } from 'vue'
import type { PartieLieutenant } from '@/lib/lieutenantRoll'
import type { MatchOutcome } from '@/types/elo'

export type PartieStep =
  | 'joueurs'
  | 'scenario'
  | 'secondaires'
  | 'lieutenant'
  | 'resultat'

export type ScenarioMode = 'list' | 'random' | 'other'

export type SecondaryDrawMode = 'draw' | 'manual'

export interface PartieScenario {
  mode: ScenarioMode
  id?: number
  slug?: string
  name?: string
  other?: string
  /** URL facultative (scénario saisi librement). */
  url?: string
}

export interface PartiePlayerSlot {
  name: string
  armyId: number
}

export interface PartieScores {
  player1Objectives: number
  player1Survivors: number
  player2Objectives: number
  player2Survivors: number
}

const STEPS: PartieStep[] = [
  'joueurs',
  'scenario',
  'secondaires',
  'lieutenant',
  'resultat',
]

export const PARTIE_STEP_LABELS: Record<PartieStep, string> = {
  joueurs: 'Joueurs',
  scenario: 'Scénario',
  secondaires: 'Secondaires',
  lieutenant: 'Jet de lieutenant',
  resultat: 'Résultat',
}

export function usePartieFlow() {
  const step = ref<PartieStep>('joueurs')
  const matchId = ref<number | null>(null)
  const player1 = ref<PartiePlayerSlot | null>(null)
  const player2 = ref<PartiePlayerSlot | null>(null)
  const scenario = ref<PartieScenario | null>(null)
  const secondaryDrawMode = ref<SecondaryDrawMode>('draw')
  const secondariesPlayer1 = ref<string[]>([])
  const secondariesPlayer2 = ref<string[]>([])
  const secondaryPool = ref<string[]>([])
  const chosenSecondaryPlayer1 = ref<string | null>(null)
  const chosenSecondaryPlayer2 = ref<string | null>(null)
  const lieutenant = ref<PartieLieutenant | null>(null)
  const scores = ref<PartieScores>({
    player1Objectives: 0,
    player1Survivors: 0,
    player2Objectives: 0,
    player2Survivors: 0,
  })

  /** Scénario libre (nom tapé) : pas d'objectifs secondaires. */
  const skipsSecondaries = computed(() => scenario.value?.mode === 'other')

  const activeSteps = computed((): PartieStep[] =>
    skipsSecondaries.value
      ? STEPS.filter((item) => item !== 'secondaires')
      : [...STEPS],
  )

  const stepIndex = computed(() => activeSteps.value.indexOf(step.value))

  const resolvedOutcome = computed((): MatchOutcome => {
    const obj1 = clampObjectives(scores.value.player1Objectives)
    const obj2 = clampObjectives(scores.value.player2Objectives)
    if (obj1 > obj2) return 'player1_win'
    if (obj2 > obj1) return 'player2_win'
    return 'draw'
  })

  function clampObjectives(value: number) {
    return Math.min(10, Math.max(0, value))
  }

  function clampSurvivors(value: number) {
    return Math.min(300, Math.max(0, value))
  }

  function canAdvanceFromJoueurs(
    p1Name?: string,
    p1Army?: string,
    p2Name?: string,
    p2Army?: string,
  ): boolean {
    return Boolean(
      p1Name &&
        p2Name &&
        p1Name !== p2Name &&
        p1Army &&
        p2Army,
    )
  }

  function setMatchId(id: number | null) {
    matchId.value = id
  }

  function setJoueurs(
    p1Name: string,
    p1ArmyId: number,
    p2Name: string,
    p2ArmyId: number,
  ) {
    player1.value = { name: p1Name, armyId: p1ArmyId }
    player2.value = { name: p2Name, armyId: p2ArmyId }
  }

  function setSecondaryDrawMode(mode: SecondaryDrawMode) {
    secondaryDrawMode.value = mode
  }

  function setScenario(value: PartieScenario) {
    scenario.value = value
    if (value.mode === 'other') {
      secondariesPlayer1.value = []
      secondariesPlayer2.value = []
      secondaryPool.value = []
      chosenSecondaryPlayer1.value = null
      chosenSecondaryPlayer2.value = null
      if (step.value === 'secondaires') {
        step.value = 'lieutenant'
      }
    }
  }

  function setSecondaries(
    p1: string[],
    p2: string[],
    chosenP1: string | null = null,
    chosenP2: string | null = null,
    pool: string[] = [],
  ) {
    secondariesPlayer1.value = p1
    secondariesPlayer2.value = p2
    chosenSecondaryPlayer1.value = chosenP1
    chosenSecondaryPlayer2.value = chosenP2
    secondaryPool.value = pool
  }

  function setLieutenant(value: PartieLieutenant) {
    lieutenant.value = value
  }

  function goTo(next: PartieStep) {
    if (skipsSecondaries.value && next === 'secondaires') {
      step.value = 'lieutenant'
      return
    }
    step.value = next
  }

  function nextStep() {
    const steps = activeSteps.value
    const index = steps.indexOf(step.value)
    if (index >= 0 && index < steps.length - 1) {
      step.value = steps[index + 1]!
    }
  }

  function prevStep() {
    const steps = activeSteps.value
    const index = steps.indexOf(step.value)
    if (index > 0) {
      step.value = steps[index - 1]!
    }
  }

  function reset() {
    step.value = 'joueurs'
    matchId.value = null
    player1.value = null
    player2.value = null
    scenario.value = null
    secondaryDrawMode.value = 'draw'
    secondariesPlayer1.value = []
    secondariesPlayer2.value = []
    secondaryPool.value = []
    chosenSecondaryPlayer1.value = null
    chosenSecondaryPlayer2.value = null
    lieutenant.value = null
    scores.value = {
      player1Objectives: 0,
      player1Survivors: 0,
      player2Objectives: 0,
      player2Survivors: 0,
    }
  }

  function scenarioPayload():
    | { scenario_id: number }
    | { scenario_other: string; scenario_url?: string }
    | undefined {
    const current = scenario.value
    if (!current) return undefined
    if (current.mode === 'other') {
      const other = current.other?.trim()
      if (!other) return undefined
      const url = current.url?.trim()
      return url ? { scenario_other: other, scenario_url: url } : { scenario_other: other }
    }
    if (current.id != null) {
      return { scenario_id: current.id }
    }
    return undefined
  }

  return {
    step,
    stepIndex,
    matchId,
    player1,
    player2,
    scenario,
    secondaryDrawMode,
    secondariesPlayer1,
    secondariesPlayer2,
    secondaryPool,
    chosenSecondaryPlayer1,
    chosenSecondaryPlayer2,
    lieutenant,
    scores,
    resolvedOutcome,
    STEPS,
    activeSteps,
    skipsSecondaries,
    clampObjectives,
    clampSurvivors,
    canAdvanceFromJoueurs,
    setMatchId,
    setJoueurs,
    setSecondaryDrawMode,
    setScenario,
    setSecondaries,
    setLieutenant,
    goTo,
    nextStep,
    prevStep,
    reset,
    scenarioPayload,
  }
}
