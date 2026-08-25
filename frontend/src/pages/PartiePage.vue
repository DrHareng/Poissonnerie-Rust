<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { ArrowLeft, Eye, Play, Trash2 } from '@lucide/vue'
import {
  fetchArmies,
  fetchMatch,
  fetchPackSecondaries,
  fetchRanking,
  fetchScenarioPack,
  deleteMatch,
  startMatch,
  updateMatchProgress,
} from '@/lib/api'
import { DEFAULT_SCENARIO_PACK_SLUG } from '@/types/elo'
import type {
  Army,
  MatchRecord,
  RankedPlayer,
  ScenarioSummary,
  SecondaryObjective,
} from '@/types/elo'
import PartieStepJoueurs from '@/components/partie/PartieStepJoueurs.vue'
import PartieStepLieutenant from '@/components/partie/PartieStepLieutenant.vue'
import PartieStepResultat from '@/components/partie/PartieStepResultat.vue'
import PartieStepScenario from '@/components/partie/PartieStepScenario.vue'
import PartieStepSecondaires from '@/components/partie/PartieStepSecondaires.vue'
import PartieStepper from '@/components/partie/PartieStepper.vue'
import SecondaryCardGrid from '@/components/partie/SecondaryCardGrid.vue'
import ScenarioDetailView from '@/components/ScenarioDetailView.vue'
import ImageViewer, { type ImageViewerItem } from '@/components/ImageViewer.vue'
import { useAuth } from '@/composables/useAuth'
import { useAppSidePanel } from '@/composables/useAppSidePanel'
import {
  usePartieFlow,
  type PartieStep,
} from '@/composables/usePartieFlow'
import type { PartieLieutenant } from '@/lib/lieutenantRoll'
import { COMBAT_ESPRIT_SLUG } from '@/lib/combatEspritDraft'
import { secondaryImageSrc } from '@/lib/secondaryImages'
import { shufflePick } from '@/lib/shufflePick'
import { formatPartieMatchup } from '@/lib/tournamentMatchDisplay'
import { externalHref } from '@/lib/utils'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

const route = useRoute()
const router = useRouter()
const { player: currentPlayer, isAuthenticated, isAdmin, login } = useAuth()
const { setCustomSide } = useAppSidePanel()

const {
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
  setMatchId,
  setJoueurs,
  setSecondaryDrawMode,
  setScenario,
  setSecondaries,
  setLieutenant,
  goTo,
  prevStep,
  nextStep,
  reset,
} = usePartieFlow()

const players = ref<RankedPlayer[]>([])
const armies = ref<Army[]>([])
const scenarios = ref<ScenarioSummary[]>([])
const secondaries = ref<SecondaryObjective[]>([])
const loadingPlayers = ref(true)
const loadingArmies = ref(true)
const loadingScenarios = ref(true)
const loadingSecondaries = ref(true)
const saving = ref(false)
const deleting = ref(false)
const apiOnline = ref(true)

const matchupLabel = computed(() => {
  if (!player1.value || !player2.value) return ''
  return formatPartieMatchup(player1.value, player2.value, armies.value, players.value)
})

function playerDisplayName(name: string): string {
  return players.value.find((player) => player.name === name)?.display_name ?? name
}

const player1DisplayName = computed(() =>
  player1.value ? playerDisplayName(player1.value.name) : '',
)

const player2DisplayName = computed(() =>
  player2.value ? playerDisplayName(player2.value.name) : '',
)

const missionMode = ref(false)
const missionImageViewerOpen = ref(false)
const missionImageViewerIndex = ref(0)
const missionImageViewerItems = ref<ImageViewerItem[]>([])

const isCombatEsprit = computed(() => scenario.value?.slug === COMBAT_ESPRIT_SLUG)

const myPlayerSlot = computed<1 | 2 | null>(() => {
  const me = currentPlayer.value?.name
  if (!me || !player1.value || !player2.value) return null
  if (player1.value.name === me) return 1
  if (player2.value.name === me) return 2
  return null
})

const secondariesReady = computed(() => {
  if (!scenario.value || myPlayerSlot.value == null) return false
  if (isCombatEsprit.value) {
    return (
      secondariesPlayer1.value.length === 3 &&
      secondariesPlayer2.value.length === 3 &&
      Boolean(chosenSecondaryPlayer1.value) &&
      Boolean(chosenSecondaryPlayer2.value)
    )
  }
  return Boolean(chosenSecondaryPlayer1.value && chosenSecondaryPlayer2.value)
})

const canShowMission = computed(
  () =>
    secondariesReady.value &&
    myPlayerSlot.value != null &&
    (step.value === 'lieutenant' ||
      step.value === 'resultat' ||
      step.value === 'secondaires'),
)

const myMissionSlugs = computed(() => {
  if (myPlayerSlot.value === 1) return secondariesPlayer1.value
  if (myPlayerSlot.value === 2) return secondariesPlayer2.value
  return []
})

const myMissionCards = computed(() =>
  myMissionSlugs.value
    .map((slug) => secondaries.value.find((item) => item.slug === slug))
    .filter((item): item is SecondaryObjective => item != null),
)

const myChosenSecondary = computed(() => {
  const slug =
    myPlayerSlot.value === 1
      ? chosenSecondaryPlayer1.value
      : myPlayerSlot.value === 2
        ? chosenSecondaryPlayer2.value
        : null
  if (!slug) return null
  return secondaries.value.find((item) => item.slug === slug) ?? null
})

const myChosenSecondaryImage = computed(() => {
  const slug = myChosenSecondary.value?.slug
  return slug ? secondaryImageSrc(slug) : undefined
})

const missionScenarioSlug = computed(() =>
  scenario.value?.mode !== 'other' ? (scenario.value?.slug ?? null) : null,
)

watch(
  [missionMode, isCombatEsprit, myChosenSecondaryImage],
  ([mode, combat, image]) => {
    setCustomSide(Boolean(mode && !combat && image))
  },
  { immediate: true },
)

watch(missionMode, (mode) => {
  if (!mode) {
    missionImageViewerOpen.value = false
  }
})

function toggleMissionMode() {
  missionMode.value = !missionMode.value
}

function openMissionSecondaryViewer(slug: string) {
  const items: ImageViewerItem[] = []
  for (const card of myMissionCards.value) {
    const src = secondaryImageSrc(card.slug)
    if (!src) continue
    items.push({ src, alt: card.name, caption: card.name })
  }
  const index = items.findIndex(
    (item) => item.src === secondaryImageSrc(slug),
  )
  if (items.length === 0) return
  missionImageViewerItems.value = items
  missionImageViewerIndex.value = Math.max(0, index)
  missionImageViewerOpen.value = true
}

function hydrateFromMatch(record: MatchRecord) {
  setMatchId(record.id)
  setJoueurs(
    record.player1,
    record.player1_army_id ?? 0,
    record.player2,
    record.player2_army_id ?? 0,
  )

  if (record.scenario_id != null || record.scenario_other) {
    const fromPack = scenarios.value.find((item) => item.id === record.scenario_id)
    setScenario(
      record.scenario_other
        ? {
            mode: 'other',
            other: record.scenario_other,
            name: record.scenario_name ?? record.scenario_other,
            ...(record.scenario_url ? { url: record.scenario_url } : {}),
          }
        : {
            mode: 'list',
            id: record.scenario_id ?? undefined,
            slug: fromPack?.slug,
            name: record.scenario_name ?? fromPack?.name,
          },
    )
  }

  {
    const p1 = record.player1_secondary_slugs ?? []
    const p2 = record.player2_secondary_slugs ?? []
    setSecondaries(
      p1,
      p2,
      record.player1_chosen_secondary ?? null,
      record.player2_chosen_secondary ?? null,
      record.secondary_pool_slugs ?? [],
    )
    // Tirage auto = exactement 3+3 ; sinon saisie manuelle (1 choisi, ou encore vide).
    setSecondaryDrawMode(p1.length === 3 && p2.length === 3 ? 'draw' : 'manual')
  }

  if (
    record.lieutenant_winner &&
    record.lieutenant_winner_choice &&
    record.lieutenant_other_choice
  ) {
    setLieutenant({
      winner: record.lieutenant_winner as PartieLieutenant['winner'],
      winnerChoice: record.lieutenant_winner_choice as PartieLieutenant['winnerChoice'],
      otherChoice: record.lieutenant_other_choice as PartieLieutenant['otherChoice'],
    })
  }

  const resumeStep = (record.partie_step as PartieStep | null) ?? 'scenario'
  if (resumeStep === 'secondaires' && record.scenario_other && !record.scenario_id) {
    goTo('lieutenant')
  } else if (STEPS.includes(resumeStep)) {
    goTo(resumeStep)
  }
}

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

    const resumeId = Number(route.params.id)
    if (Number.isFinite(resumeId) && resumeId > 0) {
      const record = await fetchMatch(resumeId)
      if (record.status !== 'in_progress') {
        toast.error('Cette partie est déjà terminée.')
        router.replace(`/matchs/${resumeId}`)
        return
      }
      hydrateFromMatch(record)
    } else {
      reset()
    }
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

function drawSecondarySlugs(): { player1: string[]; player2: string[] } {
  if (secondaries.value.length < 3) {
    throw new Error('Pas assez d’objectifs secondaires pour tirer.')
  }
  return {
    player1: shufflePick(secondaries.value, 3).map((item) => item.slug),
    player2: shufflePick(secondaries.value, 3).map((item) => item.slug),
  }
}

async function onJoueursNext(payload: {
  player1: string
  army1: number
  player2: string
  army2: number
  counts_for_elo: boolean
  secondary_draw_mode: 'draw' | 'manual'
}) {
  if (!isAuthenticated.value || !currentPlayer.value) {
    toast.error('Connectez-vous pour démarrer une partie.')
    login()
    return
  }

  const player1Name = currentPlayer.value.name
  saving.value = true
  try {
    setSecondaryDrawMode(payload.secondary_draw_mode)
    if (!matchId.value) {
      const drawn =
        payload.secondary_draw_mode === 'draw'
          ? drawSecondarySlugs()
          : { player1: [] as string[], player2: [] as string[] }
      const record = await startMatch({
        player1: player1Name,
        player2: payload.player2,
        player1_army_id: payload.army1,
        player2_army_id: payload.army2,
        player1_secondary_slugs: drawn.player1,
        player2_secondary_slugs: drawn.player2,
        counts_for_elo: payload.counts_for_elo,
      })
      setMatchId(record.id)
      setJoueurs(player1Name, payload.army1, payload.player2, payload.army2)
      setSecondaries(
        record.player1_secondary_slugs ?? drawn.player1,
        record.player2_secondary_slugs ?? drawn.player2,
      )
      await router.replace({ name: 'partie-resume', params: { id: String(record.id) } })
    } else {
      setJoueurs(player1Name, payload.army1, payload.player2, payload.army2)
    }
    nextStep()
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible de créer la partie')
  } finally {
    saving.value = false
  }
}

async function onScenarioNext(value: Parameters<typeof setScenario>[0]) {
  if (!matchId.value) return
  saving.value = true
  try {
    const isCustomScenario = value.mode === 'other'
    const isCombatEsprit = !isCustomScenario && value.slug === COMBAT_ESPRIT_SLUG
    const nextPartieStep = isCustomScenario ? 'lieutenant' : 'secondaires'
    const body: Parameters<typeof updateMatchProgress>[1] = {
      ...(isCustomScenario
        ? {
            scenario_other: value.other,
            scenario_url: value.url?.trim() || '',
          }
        : { scenario_id: value.id }),
      partie_step: nextPartieStep,
    }

    if (isCustomScenario) {
      // Pas de secondaires pour un scénario saisi librement.
      setSecondaries([], [])
    } else if (
      secondaryDrawMode.value === 'draw' &&
      !isCombatEsprit &&
      (secondariesPlayer1.value.length === 0 || secondariesPlayer2.value.length === 0)
    ) {
      // Si on revient d’un Combat de l’Esprit (tirage effacé) vers un scénario normal,
      // on retirer 3+3 et on les fige immédiatement.
      const drawn = drawSecondarySlugs()
      body.player1_secondary_slugs = drawn.player1
      body.player2_secondary_slugs = drawn.player2
      setSecondaries(drawn.player1, drawn.player2)
    }

    if (isCombatEsprit) {
      // Le tirage initial 3+3 est annulé côté serveur ; le draft le remplacera.
      if (secondariesPlayer1.value.length === 3 && secondariesPlayer2.value.length === 3) {
        setSecondaries([], [])
      }
    }

    await updateMatchProgress(matchId.value, body)
    setScenario(value)
    goTo(nextPartieStep)
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible d’enregistrer le scénario')
  } finally {
    saving.value = false
  }
}

async function onSecondairesNext(payload: {
  player1: string[]
  player2: string[]
  chosenPlayer1: string | null
  chosenPlayer2: string | null
  pool: string[] | null
}) {
  if (!matchId.value) return
  saving.value = true
  try {
    const body: Parameters<typeof updateMatchProgress>[1] = {
      player1_chosen_secondary: payload.chosenPlayer1,
      player2_chosen_secondary: payload.chosenPlayer2,
      partie_step: 'lieutenant',
    }
    // Combat de l’Esprit (ou parties anciennes) : figer le tirage s’il n’est pas encore en BDD.
    if (secondariesPlayer1.value.length === 0 || secondariesPlayer2.value.length === 0) {
      body.player1_secondary_slugs = payload.player1
      body.player2_secondary_slugs = payload.player2
    }
    if (payload.pool?.length && secondaryPool.value.length === 0) {
      body.secondary_pool_slugs = payload.pool
    }
    await updateMatchProgress(matchId.value, body)
    setSecondaries(
      payload.player1,
      payload.player2,
      payload.chosenPlayer1,
      payload.chosenPlayer2,
      payload.pool ?? [],
    )
    nextStep()
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible d’enregistrer les secondaires',
    )
  } finally {
    saving.value = false
  }
}

async function onLieutenantNext(value: Parameters<typeof setLieutenant>[0]) {
  if (!matchId.value) return
  saving.value = true
  try {
    await updateMatchProgress(matchId.value, {
      lieutenant_winner: value.winner,
      lieutenant_winner_choice: value.winnerChoice,
      lieutenant_other_choice: value.otherChoice,
      partie_step: 'resultat',
    })
    setLieutenant(value)
    nextStep()
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible d’enregistrer le jet de lieutenant',
    )
  } finally {
    saving.value = false
  }
}

function updateScores(value: typeof scores.value) {
  scores.value = value
}

function abandonPartie() {
  if (stepIndex.value === 0 && !matchId.value) {
    router.push('/classement')
    return
  }
  if (window.confirm('Quitter cette partie ? Elle restera disponible dans vos parties en cours.')) {
    const id = matchId.value
    reset()
    router.push(id ? '/matchs' : '/classement')
  }
}

async function deletePartie() {
  if (!isAdmin.value || !matchId.value) return
  if (!window.confirm('Supprimer définitivement cette partie en cours ?')) return

  deleting.value = true
  try {
    await deleteMatch(matchId.value)
    toast.success('Partie supprimée')
    reset()
    router.push('/matchs')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Suppression impossible')
  } finally {
    deleting.value = false
  }
}

onMounted(loadData)
</script>

<template>
  <div class="page-stack">
    <section class="page-header">
      <div class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div class="space-y-2">
          <div class="flex flex-wrap items-center gap-3">
            <h1 class="page-title">
              Partie<span v-if="matchId"> #{{ matchId }}</span>
            </h1>
            <Button
              v-if="canShowMission"
              type="button"
              size="sm"
              :variant="missionMode ? 'outline' : 'default'"
              @click="toggleMissionMode"
            >
              <ArrowLeft v-if="missionMode" class="size-4" />
              <Eye v-else class="size-4" />
              {{ missionMode ? 'Retour à la saisie' : 'Ma mission' }}
            </Button>
          </div>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <Button
            v-if="isAdmin && matchId"
            type="button"
            variant="destructive"
            size="sm"
            :disabled="deleting"
            @click="deletePartie"
          >
            <Trash2 class="size-4" />
            {{ deleting ? 'Suppression…' : 'Supprimer' }}
          </Button>
          <Button type="button" variant="outline" size="sm" @click="abandonPartie">
            Quitter
          </Button>
        </div>
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

    <Teleport defer to="#app-side-panel">
      <div
        v-if="missionMode && !isCombatEsprit && myChosenSecondaryImage"
        class="flex h-full min-h-0 flex-col items-center justify-center gap-3 p-2"
      >
        <img
          :src="myChosenSecondaryImage"
          :alt="myChosenSecondary?.name ?? 'Objectif secondaire'"
          class="poissonnerie-side-image max-h-full w-auto cursor-zoom-in object-contain"
          @click="
            openMissionSecondaryViewer(myChosenSecondary?.slug ?? '')
          "
        />
        <p
          v-if="myChosenSecondary"
          class="text-center font-display text-sm font-semibold tracking-wide text-foreground"
        >
          {{ myChosenSecondary.name }}
        </p>
      </div>
    </Teleport>

    <template v-if="missionMode">
      <div class="page-panel-scroll min-h-0 flex-1 overflow-y-auto">
        <template v-if="isCombatEsprit">
          <Card class="neon-panel">
            <CardHeader>
              <CardTitle>Ma mission</CardTitle>
              <CardDescription>
                Vos trois objectifs secondaires — Le combat de l'esprit
              </CardDescription>
            </CardHeader>
            <CardContent>
              <SecondaryCardGrid
                v-if="myMissionCards.length > 0"
                :secondaries="myMissionCards"
                viewable
                @view="openMissionSecondaryViewer"
              />
              <p v-else class="text-sm text-muted-foreground">
                Objectifs secondaires indisponibles.
              </p>
            </CardContent>
          </Card>
        </template>
        <template v-else-if="missionScenarioSlug">
          <ScenarioDetailView :slug="missionScenarioSlug" />
        </template>
        <Card v-else class="neon-panel">
          <CardHeader>
            <CardTitle>Ma mission</CardTitle>
          </CardHeader>
          <CardContent class="space-y-2">
            <p class="text-sm text-muted-foreground">
              {{
                scenario?.name || scenario?.other || 'Scénario personnalisé'
              }}
              — détail du pack indisponible pour ce scénario.
            </p>
            <a
              v-if="scenario?.url"
              :href="externalHref(scenario.url)"
              target="_blank"
              rel="noopener noreferrer"
              class="inline-block text-sm font-medium text-primary hover:underline"
            >
              Voir le scénario
            </a>
          </CardContent>
        </Card>
      </div>
    </template>

    <template v-else>
      <PartieStepper class="shrink-0" :steps="activeSteps" :current-index="stepIndex" />

      <div class="page-panel-scroll min-h-0 flex-1 overflow-y-auto">
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
                      : step === 'lieutenant'
                        ? 'Jet de lieutenant'
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
              :loading="loadingPlayers || saving"
              :armies-loading="loadingArmies"
              :locked-player1-name="currentPlayer?.name"
              :initial-player1="player1?.name"
              :initial-player2="player2?.name"
              :initial-army1="player1?.armyId"
              :initial-army2="player2?.armyId"
              :initial-secondary-draw-mode="secondaryDrawMode"
              @next="onJoueursNext"
            />

            <PartieStepScenario
              v-else-if="step === 'scenario'"
              :scenarios="scenarios"
              :loading="loadingScenarios || saving"
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
              :loading="loadingSecondaries || saving"
              :manual-selection="secondaryDrawMode === 'manual'"
              :initial-player1="secondariesPlayer1"
              :initial-player2="secondariesPlayer2"
              :initial-chosen-player1="chosenSecondaryPlayer1"
              :initial-chosen-player2="chosenSecondaryPlayer2"
              :initial-pool="secondaryPool"
              @back="prevStep"
              @next="onSecondairesNext"
            />

            <PartieStepLieutenant
              v-else-if="step === 'lieutenant' && player1 && player2"
              :player1-display-name="player1DisplayName"
              :player2-display-name="player2DisplayName"
              :initial="lieutenant"
              @back="prevStep"
              @next="onLieutenantNext"
            />

            <PartieStepResultat
              v-else-if="step === 'resultat' && player1 && player2 && scenario && matchId"
              :match-id="matchId"
              :player1="player1"
              :player2="player2"
              :scenario="scenario"
              :scores="scores"
              :resolved-outcome="resolvedOutcome"
              @back="prevStep"
              @update:scores="updateScores"
              @recorded="reset"
            />
          </CardContent>
        </Card>
      </div>
    </template>

    <ImageViewer
      v-model:open="missionImageViewerOpen"
      v-model:index="missionImageViewerIndex"
      :items="missionImageViewerItems"
    />
  </div>
</template>
