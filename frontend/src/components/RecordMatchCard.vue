<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { Swords } from '@lucide/vue'
import { fetchArmies, fetchScenarios, recordMatch } from '@/lib/api'
import type { Army, MatchOutcome, RankedPlayer, Scenario } from '@/types/elo'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import SectorialPicker from '@/components/SectorialPicker.vue'
import PlayerPicker from '@/components/PlayerPicker.vue'

const SCENARIO_OTHER_VALUE = '__other__'

const props = defineProps<{
  players: RankedPlayer[]
  loading?: boolean
}>()

const emit = defineEmits<{
  recorded: []
  cancel: []
}>()

const player1 = ref<string>()
const player2 = ref<string>()
const player1ArmyId = ref<string>()
const player2ArmyId = ref<string>()
const armies = ref<Army[]>([])
const armiesLoading = ref(true)
const scenarios = ref<Scenario[]>([])
const scenariosLoading = ref(true)
const player1Objectives = ref(0)
const player1Survivors = ref(0)
const player2Objectives = ref(0)
const player2Survivors = ref(0)
const scenarioChoice = ref<string>()
const scenarioOther = ref('')
const submitting = ref(false)

const playerOptions = computed(() =>
  props.players.map((player) => ({
    label: `${player.display_name} (${Math.round(player.rating)})`,
    value: player.name,
  })),
)

const armyOptions = computed(() => armies.value)

const isScenarioOther = computed(
  () => scenarioChoice.value === SCENARIO_OTHER_VALUE,
)

const bothPlayersSelected = computed(
  () =>
    Boolean(player1.value && player2.value) &&
    player1.value !== player2.value,
)

const player1Options = computed(() =>
  playerOptions.value.filter((option) => option.value !== player2.value),
)

const player2Options = computed(() =>
  playerOptions.value.filter((option) => option.value !== player1.value),
)

const player1ArmyEnabled = computed(() => Boolean(player1.value))
const player2ArmyEnabled = computed(() => Boolean(player2.value))

const player1ScoresEnabled = computed(
  () => Boolean(player1.value && player1ArmyId.value),
)
const player2ScoresEnabled = computed(
  () => Boolean(player2.value && player2ArmyId.value),
)

const canSubmit = computed(
  () =>
    bothPlayersSelected.value &&
    Boolean(player1ArmyId.value && player2ArmyId.value),
)

const clampedObjectives = computed(() => ({
  player1: clampObjectives(player1Objectives.value),
  player2: clampObjectives(player2Objectives.value),
}))

const resolvedOutcome = computed((): MatchOutcome => {
  const { player1: obj1, player2: obj2 } = clampedObjectives.value
  if (obj1 > obj2) return 'player1_win'
  if (obj2 > obj1) return 'player2_win'
  return 'draw'
})

const submitLabel = computed(() => {
  if (!bothPlayersSelected.value) return 'Valider le match'

  if (resolvedOutcome.value === 'player1_win') {
    return victoryLabel(player1.value!)
  }
  if (resolvedOutcome.value === 'player2_win') {
    return victoryLabel(player2.value!)
  }
  return 'Valider le match nul'
})

function victoryLabel(name: string) {
  const first = name.trim().charAt(0).toLowerCase()
  if ('aeiouhàâäéèêëïîôùûü'.includes(first)) {
    return `Valider la victoire d'${name}`
  }
  return `Valider la victoire de ${name}`
}

function clampObjectives(value: number) {
  return Math.min(10, Math.max(0, value))
}

function clampSurvivors(value: number) {
  return Math.min(300, Math.max(0, value))
}

function resetPlayer1Details() {
  player1ArmyId.value = undefined
  player1Objectives.value = 0
  player1Survivors.value = 0
}

function resetPlayer2Details() {
  player2ArmyId.value = undefined
  player2Objectives.value = 0
  player2Survivors.value = 0
}

function resetScenario() {
  scenarioChoice.value = undefined
  scenarioOther.value = ''
}

function resetForm() {
  player1.value = undefined
  player2.value = undefined
  resetPlayer1Details()
  resetPlayer2Details()
  resetScenario()
}

function cancel() {
  resetForm()
  emit('cancel')
}

function scenarioPayload():
  | { scenario_id: number }
  | { scenario_other: string }
  | undefined {
  if (!scenarioChoice.value) return undefined
  if (scenarioChoice.value === SCENARIO_OTHER_VALUE) {
    const other = scenarioOther.value.trim()
    return other ? { scenario_other: other } : undefined
  }
  const id = Number(scenarioChoice.value)
  return Number.isFinite(id) ? { scenario_id: id } : undefined
}

onMounted(async () => {
  try {
    const [loadedArmies, loadedScenarios] = await Promise.all([
      fetchArmies(),
      fetchScenarios(),
    ])
    armies.value = loadedArmies
    scenarios.value = loadedScenarios
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de charger les armées. Lancez poissonnerie-sync-armies.',
    )
  } finally {
    armiesLoading.value = false
    scenariosLoading.value = false
  }
})

watch(player1, () => {
  resetPlayer1Details()
})

watch(player2, () => {
  resetPlayer2Details()
})

watch(player1ArmyId, () => {
  player1Objectives.value = 0
  player1Survivors.value = 0
})

watch(player2ArmyId, () => {
  player2Objectives.value = 0
  player2Survivors.value = 0
})

watch(scenarioChoice, (value) => {
  if (value !== SCENARIO_OTHER_VALUE) {
    scenarioOther.value = ''
  }
})

function parseScores() {
  return {
    player1_objectives: clampObjectives(player1Objectives.value),
    player1_survivors: clampSurvivors(player1Survivors.value),
    player2_objectives: clampObjectives(player2Objectives.value),
    player2_survivors: clampSurvivors(player2Survivors.value),
  }
}

async function submit() {
  if (!player1.value || !player2.value) {
    toast.error('Sélectionnez deux joueurs.')
    return
  }

  if (player1.value === player2.value) {
    toast.error('Un joueur ne peut pas jouer contre lui-même.')
    return
  }

  if (!player1ArmyId.value || !player2ArmyId.value) {
    toast.error('Sélectionnez une armée pour chaque joueur.')
    return
  }

  submitting.value = true
  try {
    const scores = parseScores()
    const record = await recordMatch(
      player1.value,
      player2.value,
      resolvedOutcome.value,
      scores,
      {
        player1_army_id: Number(player1ArmyId.value),
        player2_army_id: Number(player2ArmyId.value),
      },
      scenarioPayload(),
    )
    toast.success(
      `${record.player1} ${Math.round(record.player1_old)} → ${Math.round(record.player1_new)} | ` +
        `${record.player2} ${Math.round(record.player2_old)} → ${Math.round(record.player2_new)}`,
    )
    player1.value = undefined
    player2.value = undefined
    resetForm()
    emit('recorded')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur inconnue')
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <Card class="neon-panel">
    <CardHeader>
      <CardTitle class="flex items-center gap-2">
        <Swords class="size-5 text-primary" />
        Enregistrer un match
      </CardTitle>
      <CardDescription>
        Le classement ELO est recalculé automatiquement après chaque partie.
      </CardDescription>
    </CardHeader>
    <CardContent>
      <div class="grid gap-4">
        <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <section class="player-match-panel">
            <p class="player-match-panel-title">Joueur 1</p>

            <div class="grid gap-2">
              <Label>Joueur</Label>
              <PlayerPicker
                v-model="player1"
                :options="player1Options"
                :disabled="loading || playerOptions.length < 2"
                placeholder="Tapez pour chercher un joueur"
              />
            </div>

            <div class="grid gap-2">
              <Label>Sectorielle</Label>
              <SectorialPicker
                v-model="player1ArmyId"
                :armies="armyOptions"
                :disabled="!player1ArmyEnabled || armiesLoading || armyOptions.length === 0"
                :placeholder="
                  player1ArmyEnabled
                    ? 'Tapez pour chercher une sectorielle'
                    : 'Sélectionnez d\'abord le joueur'
                "
              />
            </div>

            <div class="grid gap-3">
              <div class="grid gap-2">
                <Label :for="`p1-objectives`">Points d'objectifs (0–10)</Label>
                <Input
                  :id="`p1-objectives`"
                  v-model.number="player1Objectives"
                  type="number"
                  min="0"
                  max="10"
                  step="1"
                  :disabled="!player1ScoresEnabled"
                />
              </div>
              <div class="grid gap-2">
                <Label :for="`p1-survivors`">Points de survivants (0–300)</Label>
                <Input
                  :id="`p1-survivors`"
                  v-model.number="player1Survivors"
                  type="number"
                  min="0"
                  max="300"
                  step="1"
                  :disabled="!player1ScoresEnabled"
                />
              </div>
            </div>
          </section>

          <section class="player-match-panel">
            <p class="player-match-panel-title">Joueur 2</p>

            <div class="grid gap-2">
              <Label>Joueur</Label>
              <PlayerPicker
                v-model="player2"
                :options="player2Options"
                :disabled="loading || playerOptions.length < 2"
                placeholder="Tapez pour chercher un joueur"
              />
            </div>

            <div class="grid gap-2">
              <Label>Sectorielle</Label>
              <SectorialPicker
                v-model="player2ArmyId"
                :armies="armyOptions"
                :disabled="!player2ArmyEnabled || armiesLoading || armyOptions.length === 0"
                :placeholder="
                  player2ArmyEnabled
                    ? 'Tapez pour chercher une sectorielle'
                    : 'Sélectionnez d\'abord le joueur'
                "
              />
            </div>

            <div class="grid gap-3">
              <div class="grid gap-2">
                <Label :for="`p2-objectives`">Points d'objectifs (0–10)</Label>
                <Input
                  :id="`p2-objectives`"
                  v-model.number="player2Objectives"
                  type="number"
                  min="0"
                  max="10"
                  step="1"
                  :disabled="!player2ScoresEnabled"
                />
              </div>
              <div class="grid gap-2">
                <Label :for="`p2-survivors`">Points de survivants (0–300)</Label>
                <Input
                  :id="`p2-survivors`"
                  v-model.number="player2Survivors"
                  type="number"
                  min="0"
                  max="300"
                  step="1"
                  :disabled="!player2ScoresEnabled"
                />
              </div>
            </div>
          </section>
        </div>

        <div class="grid gap-2">
          <Label>Scénario (optionnel)</Label>
          <Select v-model="scenarioChoice" :disabled="scenariosLoading">
            <SelectTrigger>
              <SelectValue placeholder="Choisir un scénario" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="scenario in scenarios"
                :key="scenario.id"
                :value="String(scenario.id)"
              >
                {{ scenario.name }}
              </SelectItem>
              <SelectItem :value="SCENARIO_OTHER_VALUE">Autre…</SelectItem>
            </SelectContent>
          </Select>
          <Input
            v-if="isScenarioOther"
            id="scenario-other"
            v-model="scenarioOther"
            placeholder="Nom du scénario"
            autocomplete="off"
          />
        </div>

        <div class="flex flex-col gap-2 sm:flex-row sm:justify-end">
          <Button
            v-if="canSubmit"
            type="button"
            :disabled="loading || submitting"
            @click="submit"
          >
            {{ submitting ? 'Enregistrement...' : submitLabel }}
          </Button>
          <Button
            type="button"
            variant="outline"
            :disabled="submitting"
            @click="cancel"
          >
            Annuler
          </Button>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
