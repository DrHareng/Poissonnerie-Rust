<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { Play } from '@lucide/vue'
import {
  fetchArmies,
  fetchPackSecondaries,
  fetchRanking,
  fetchScenarioPack,
} from '@/lib/api'
import { DEFAULT_SCENARIO_PACK_SLUG } from '@/types/elo'
import type { Army, RankedPlayer, ScenarioSummary, SecondaryObjective } from '@/types/elo'
import PartieStepJoueurs from '@/components/partie/PartieStepJoueurs.vue'
import PartieStepResultat from '@/components/partie/PartieStepResultat.vue'
import PartieStepScenario from '@/components/partie/PartieStepScenario.vue'
import PartieStepSecondaires from '@/components/partie/PartieStepSecondaires.vue'
import PartieStepper from '@/components/partie/PartieStepper.vue'
import { useAuth } from '@/composables/useAuth'
import { usePartieFlow } from '@/composables/usePartieFlow'
import { formatPartieMatchup } from '@/lib/tournamentMatchDisplay'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

const router = useRouter()
const { player: currentPlayer, isAuthenticated, login } = useAuth()

const {
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
  setJoueurs,
  setScenario,
  setSecondaries,
  prevStep,
  nextStep,
  reset,
  scenarioPayload,
} = usePartieFlow()

const players = ref<RankedPlayer[]>([])
const armies = ref<Army[]>([])
const scenarios = ref<ScenarioSummary[]>([])
const secondaries = ref<SecondaryObjective[]>([])
const loadingPlayers = ref(true)
const loadingArmies = ref(true)
const loadingScenarios = ref(true)
const loadingSecondaries = ref(true)
const apiOnline = ref(true)

const matchupLabel = computed(() => {
  if (!player1.value || !player2.value) return ''
  return formatPartieMatchup(player1.value, player2.value, armies.value, players.value)
})

async function loadData() {
  loadingPlayers.value = true
  loadingArmies.value = true
  loadingScenarios.value = true
  loadingSecondaries.value = true

  try {
    const [loadedPlayers, loadedArmies, pack, loadedSecondaries] = await Promise.all([
      fetchRanking(),
      fetchArmies(),
      fetchScenarioPack(DEFAULT_SCENARIO_PACK_SLUG),
      fetchPackSecondaries(DEFAULT_SCENARIO_PACK_SLUG),
    ])
    players.value = loadedPlayers
    armies.value = loadedArmies
    scenarios.value = pack.scenarios
    secondaries.value = loadedSecondaries
    apiOnline.value = true
  } catch (error) {
    apiOnline.value = false
    toast.error(
      error instanceof Error ? error.message : 'Impossible de charger les données',
    )
  } finally {
    loadingPlayers.value = false
    loadingArmies.value = false
    loadingScenarios.value = false
    loadingSecondaries.value = false
  }
}

function onJoueursNext(payload: {
  player1: string
  army1: number
  player2: string
  army2: number
}) {
  const player1Name = currentPlayer.value?.name ?? payload.player1
  setJoueurs(player1Name, payload.army1, payload.player2, payload.army2)
  nextStep()
}

function onScenarioNext(value: Parameters<typeof setScenario>[0]) {
  setSecondaries([], [])
  setScenario(value)
  nextStep()
}

function onSecondairesNext(payload: { player1: string[]; player2: string[] }) {
  setSecondaries(payload.player1, payload.player2)
  nextStep()
}

function updateScores(value: typeof scores.value) {
  scores.value = value
}

function abandonPartie() {
  if (stepIndex.value === 0) {
    router.push('/classement')
    return
  }
  if (window.confirm('Abandonner cette partie en cours ?')) {
    reset()
    router.push('/classement')
  }
}

onMounted(loadData)
</script>

<template>
  <div class="page-stack">
    <section class="page-header">
      <div class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div class="space-y-2">
          <h1 class="page-title">Partie</h1>
          <p class="page-description">
            Assistant pas à pas pour préparer et enregistrer une partie entre deux
            joueurs.
          </p>
        </div>
        <Button type="button" variant="outline" size="sm" @click="abandonPartie">
          Abandonner
        </Button>
      </div>
    </section>

    <Alert v-if="!apiOnline" variant="destructive" class="neon-panel-accent">
      <AlertTitle>API indisponible</AlertTitle>
      <AlertDescription>
        Lancez le serveur Rust avec
        <code class="rounded bg-muted px-1 py-0.5">cargo run --bin poissonnerie-server</code>
        puis rechargez la page.
      </AlertDescription>
    </Alert>

    <Alert
      v-if="!isAuthenticated || !currentPlayer"
      variant="destructive"
      class="neon-panel-accent"
    >
      <AlertTitle>Connexion requise</AlertTitle>
      <AlertDescription>
        Le joueur 1 est toujours votre profil. Connectez-vous avec Discord et assurez-vous
        qu'un joueur Poissonnerie est bien lié à votre compte.
        <Button
          v-if="!isAuthenticated"
          type="button"
          size="sm"
          class="ml-3"
          @click="login"
        >
          Connexion Discord
        </Button>
      </AlertDescription>
    </Alert>

    <PartieStepper :steps="STEPS" :current-index="stepIndex" />

    <Card class="neon-panel">
      <CardHeader>
        <CardTitle class="flex items-center gap-2">
          <Play class="size-5 text-primary" />
          {{
            step === 'joueurs'
              ? 'Joueurs et sectorielles'
              : step === 'scenario'
                ? 'Choix du scénario'
                : step === 'secondaires'
                  ? 'Objectifs secondaires'
                  : 'Résultat'
          }}
        </CardTitle>
        <CardDescription v-if="player1 && player2 && step !== 'joueurs'">
          {{ matchupLabel }}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <PartieStepJoueurs
          v-if="step === 'joueurs'"
          :players="players"
          :armies="armies"
          :loading="loadingPlayers"
          :armies-loading="loadingArmies"
          :locked-player1-name="currentPlayer?.name"
          :initial-player1="player1?.name"
          :initial-player2="player2?.name"
          :initial-army1="player1?.armyId"
          :initial-army2="player2?.armyId"
          @next="onJoueursNext"
        />

        <PartieStepScenario
          v-else-if="step === 'scenario'"
          :scenarios="scenarios"
          :loading="loadingScenarios"
          :initial="scenario"
          @back="prevStep"
          @next="onScenarioNext"
        />

        <PartieStepSecondaires
          v-else-if="step === 'secondaires' && player1 && player2"
          :player1-name="player1.name"
          :player2-name="player2.name"
          :scenario-slug="scenario?.slug"
          :secondaries="secondaries"
          :loading="loadingSecondaries"
          :initial-player1="secondariesPlayer1"
          :initial-player2="secondariesPlayer2"
          @back="prevStep"
          @next="onSecondairesNext"
        />

        <PartieStepResultat
          v-else-if="step === 'resultat' && player1 && player2 && scenario"
          :player1="player1"
          :player2="player2"
          :scenario="scenario"
          :scores="scores"
          :resolved-outcome="resolvedOutcome"
          :scenario-payload="scenarioPayload"
          @back="prevStep"
          @update:scores="updateScores"
          @recorded="reset"
        />
      </CardContent>
    </Card>
  </div>
</template>
