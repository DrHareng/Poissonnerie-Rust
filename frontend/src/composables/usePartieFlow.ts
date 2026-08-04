import { computed, ref } from 'vue'
import type { MatchOutcome } from '@/types/elo'

export type PartieStep = 'joueurs' | 'scenario' | 'secondaires' | 'resultat'

export type ScenarioMode = 'list' | 'random' | 'other'

export interface PartieScenario {
  mode: ScenarioMode
  id?: number
  slug?: string
  name?: string
  other?: string
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

const STEPS: PartieStep[] = ['joueurs', 'scenario', 'secondaires', 'resultat']

export const PARTIE_STEP_LABELS: Record<PartieStep, string> = {
  joueurs: 'Joueurs',
  scenario: 'Scénario',
  secondaires: 'Secondaires',
  resultat: 'Résultat',
}

export function usePartieFlow() {
  const step = ref<PartieStep>('joueurs')
  const player1 = ref<PartiePlayerSlot | null>(null)
  const player2 = ref<PartiePlayerSlot | null>(null)
  const scenario = ref<PartieScenario | null>(null)
  const secondariesPlayer1 = ref<string[]>([])
  const secondariesPlayer2 = ref<string[]>([])
  const scores = ref<PartieScores>({
    player1Objectives: 0,
    player1Survivors: 0,
    player2Objectives: 0,
    player2Survivors: 0,
  })

  const stepIndex = computed(() => STEPS.indexOf(step.value))

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

  function setJoueurs(
    p1Name: string,
    p1ArmyId: number,
    p2Name: string,
    p2ArmyId: number,
  ) {
    player1.value = { name: p1Name, armyId: p1ArmyId }
    player2.value = { name: p2Name, armyId: p2ArmyId }
  }

  function setScenario(value: PartieScenario) {
    scenario.value = value
  }

  function setSecondaries(p1: string[], p2: string[]) {
    secondariesPlayer1.value = p1
    secondariesPlayer2.value = p2
  }

  function goTo(next: PartieStep) {
    step.value = next
  }

  function nextStep() {
    const index = stepIndex.value
    if (index < STEPS.length - 1) {
      step.value = STEPS[index + 1]!
    }
  }

  function prevStep() {
    const index = stepIndex.value
    if (index > 0) {
      step.value = STEPS[index - 1]!
    }
  }

  function reset() {
    step.value = 'joueurs'
    player1.value = null
    player2.value = null
    scenario.value = null
    secondariesPlayer1.value = []
    secondariesPlayer2.value = []
    scores.value = {
      player1Objectives: 0,
      player1Survivors: 0,
      player2Objectives: 0,
      player2Survivors: 0,
    }
  }

  function scenarioPayload():
    | { scenario_id: number }
    | { scenario_other: string }
    | undefined {
    const current = scenario.value
    if (!current) return undefined
    if (current.mode === 'other') {
      const other = current.other?.trim()
      return other ? { scenario_other: other } : undefined
    }
    if (current.id != null) {
      return { scenario_id: current.id }
    }
    return undefined
  }

  return {
    step,
    stepIndex,
    player1,
    player2,
    scenario,
    secondariesPlayer1,
    secondariesPlayer2,
    scores,
    resolvedOutcome,
    STEPS,
    clampObjectives,
    clampSurvivors,
    canAdvanceFromJoueurs,
    setJoueurs,
    setScenario,
    setSecondaries,
    goTo,
    nextStep,
    prevStep,
    reset,
    scenarioPayload,
  }
}
