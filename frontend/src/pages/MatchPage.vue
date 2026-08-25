<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { Play, Swords, Trash2 } from '@lucide/vue'
import {
  deleteMatch,
  fetchArmies,
  fetchMatch,
  fetchPackSecondaries,
  fetchScenarioPack,
  updateMatchArmyList,
  updateMatchReport,
} from '@/lib/api'
import type {
  Army,
  MatchRecord,
  ScenarioSummary,
  SecondaryObjective,
} from '@/types/elo'
import { DEFAULT_SCENARIO_PACK_SLUG } from '@/types/elo'
import ArmyLogo from '@/components/ArmyLogo.vue'
import MatchArmyListCard from '@/components/MatchArmyListCard.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import AdminContentEditor from '@/components/AdminContentEditor.vue'
import ContentHoverTip from '@/components/ContentHoverTip.vue'
import CombatEspritPlayerHand from '@/components/partie/CombatEspritPlayerHand.vue'
import { useAuth } from '@/composables/useAuth'
import { COMBAT_ESPRIT_SLUG, draftStepBadges } from '@/lib/combatEspritDraft'
import { secondaryImageSrc } from '@/lib/secondaryImages'
import { choiceLabel } from '@/lib/lieutenantRoll'
import { casualMatchContextLabel, matchCountsForElo } from '@/lib/matchElo'
import { formatMatchRecordedDate } from '@/lib/tournamentMatchDisplay'
import { phaseLabel } from '@/lib/tournamentPhase'
import { matchsTabs } from '@/lib/pageTitleTabs'
import { externalHref } from '@/lib/utils'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
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

const match = ref<MatchRecord | null>(null)
const armies = ref<Army[]>([])
const secondaries = ref<SecondaryObjective[]>([])
const scenarios = ref<ScenarioSummary[]>([])
const loading = ref(true)
const deleting = ref(false)
const apiOnline = ref(true)

const matchId = computed(() => Number(route.params.id))

function armyName(armyId?: number | null): string {
  if (!armyId) return 'Sectorielle'
  return armies.value.find((army) => army.id === armyId)?.name ?? 'Sectorielle'
}

function secondaryBySlug(slug: string | null | undefined): SecondaryObjective | null {
  if (!slug) return null
  return secondaries.value.find((item) => item.slug === slug) ?? null
}

const packScenario = computed(() => {
  if (!match.value?.scenario_id) return null
  return scenarios.value.find((item) => item.id === match.value!.scenario_id) ?? null
})

const scenarioLabel = computed(() => {
  if (!match.value) return null
  return (
    packScenario.value?.name ??
    match.value.scenario_name ??
    match.value.scenario_other ??
    null
  )
})

const canResume = computed(() => {
  if (!match.value || match.value.status !== 'in_progress') return false
  if (!currentPlayer.value) return false
  const me = currentPlayer.value.name
  return (
    match.value.created_by === me ||
    match.value.player1 === me ||
    match.value.player2 === me
  )
})

const matchupLabel = computed(() => {
  if (!match.value) return ''
  const p1 = match.value.player1_display_name ?? match.value.player1
  const p2 = match.value.player2_display_name ?? match.value.player2
  return `${p1} (${armyName(match.value.player1_army_id)}) vs ${p2} (${armyName(match.value.player2_army_id)})`
})

const matchDateLabel = computed(
  () => formatMatchRecordedDate(match.value?.recorded_at ?? 0) ?? 'Date inconnue',
)

const tournamentPhaseText = computed(() => phaseLabel(match.value?.tournament_phase))

const showsElo = computed(() => matchCountsForElo(match.value?.counts_for_elo))

const casualContextLabel = computed(() =>
  casualMatchContextLabel(match.value?.counts_for_elo),
)

const lieutenantLines = computed(() => {
  const record = match.value
  if (
    !record?.lieutenant_winner ||
    !record.lieutenant_winner_choice ||
    !record.lieutenant_other_choice
  ) {
    return null
  }

  const winnerName =
    record.lieutenant_winner === 'player1'
      ? (record.player1_display_name ?? record.player1)
      : (record.player2_display_name ?? record.player2)
  const otherName =
    record.lieutenant_winner === 'player1'
      ? (record.player2_display_name ?? record.player2)
      : (record.player1_display_name ?? record.player1)

  return [
    `${winnerName} gagne le jet de lieutenant`,
    `${winnerName} ${choiceLabel(record.lieutenant_winner_choice)}`,
    `${otherName} ${choiceLabel(record.lieutenant_other_choice)}`,
  ]
})

const outcomeLine = computed(() => {
  if (!match.value) return null
  if (match.value.status === 'in_progress' || !match.value.outcome) {
    return 'Partie en cours'
  }
  if (match.value.outcome === 'draw') return 'Match nul'
  const winner =
    match.value.outcome === 'player1_win'
      ? (match.value.player1_display_name ?? match.value.player1)
      : (match.value.player2_display_name ?? match.value.player2)
  return `Victoire de ${winner}`
})

function isWinner(slot: 'player1' | 'player2'): boolean {
  if (!match.value || match.value.status === 'in_progress' || !match.value.outcome) {
    return false
  }
  if (match.value.outcome === 'draw') return false
  return (
    (slot === 'player1' && match.value.outcome === 'player1_win') ||
    (slot === 'player2' && match.value.outcome === 'player2_win')
  )
}

function objectivesPoints(slot: 'player1' | 'player2'): number | null {
  if (!match.value || match.value.status === 'in_progress') return null
  return slot === 'player1'
    ? match.value.player1_objectives
    : match.value.player2_objectives
}

function survivorsPoints(slot: 'player1' | 'player2'): number | null {
  if (!match.value || match.value.status === 'in_progress') return null
  return slot === 'player1'
    ? match.value.player1_survivors
    : match.value.player2_survivors
}

function canEditReport(playerName: string): boolean {
  return Boolean(
    isAuthenticated.value &&
      currentPlayer.value &&
      currentPlayer.value.name.localeCompare(playerName, undefined, {
        sensitivity: 'accent',
      }) === 0,
  )
}

function canEditArmyList(playerName: string): boolean {
  if (match.value?.tournament_id != null) return false
  return canEditReport(playerName)
}

const isTournamentMatch = computed(() => match.value?.tournament_id != null)

function armyListHidden(code: string | null | undefined) {
  return isTournamentMatch.value && !code?.trim()
}

function eloDelta(oldRating: number, newRating: number): string {
  const delta = Math.round(newRating - oldRating)
  if (delta > 0) return `+${delta}`
  return String(delta)
}

function reportDateLabel(createdAt?: number | null, updatedAt?: number | null): string | null {
  const ts = updatedAt || createdAt
  if (!ts) return null
  const formatted = formatMatchRecordedDate(ts)
  if (!formatted) return null
  if (updatedAt && createdAt && updatedAt !== createdAt) {
    return `Mis à jour le ${formatted}`
  }
  return `Saisi le ${formatted}`
}

function secondaryLabel(slug: string | null | undefined): string {
  if (!slug) return '—'
  return secondaryBySlug(slug)?.name ?? slug
}

function secondaryBody(slug: string | null | undefined): string {
  if (!slug) return ''
  return secondaryBySlug(slug)?.body_md ?? ''
}

function resolveSecondary(slug: string | null | undefined): SecondaryObjective | null {
  return secondaryBySlug(slug)
}

function handPicks(slugs: string[] | null | undefined): Array<SecondaryObjective | null> {
  return [0, 1, 2].map((index) => resolveSecondary(slugs?.[index] ?? null))
}

const isCombatEspritMatch = computed(
  () =>
    Boolean(match.value?.secondary_pool_slugs?.length) ||
    packScenario.value?.slug === COMBAT_ESPRIT_SLUG,
)

/** Affichage match : player1 = starter (A). */
const player1HandBadges = computed(() => draftStepBadges('A'))
const player2HandBadges = computed(() => draftStepBadges('B'))

async function loadMatch() {
  loading.value = true
  try {
    const [loadedMatch, loadedArmies, loadedSecondaries, pack] = await Promise.all([
      fetchMatch(matchId.value),
      fetchArmies(),
      fetchPackSecondaries(DEFAULT_SCENARIO_PACK_SLUG),
      fetchScenarioPack(DEFAULT_SCENARIO_PACK_SLUG),
    ])
    match.value = loadedMatch
    armies.value = loadedArmies
    secondaries.value = loadedSecondaries
    scenarios.value = pack.scenarios
    apiOnline.value = true
  } catch (error) {
    apiOnline.value = false
    match.value = null
    toast.error(error instanceof Error ? error.message : 'Match introuvable')
  } finally {
    loading.value = false
  }
}

function resumePartie() {
  if (!match.value) return
  router.push({ name: 'partie-resume', params: { id: String(match.value.id) } })
}

async function persistPlayer1Report(payload: { body: string }) {
  if (!match.value) return
  match.value = await updateMatchReport(match.value.id, payload.body)
  toast.success('Compte rendu enregistré')
}

async function persistPlayer2Report(payload: { body: string }) {
  if (!match.value) return
  match.value = await updateMatchReport(match.value.id, payload.body)
  toast.success('Compte rendu enregistré')
}

async function persistPlayer1ArmyList(code: string, armyId?: number | null) {
  if (!match.value) return
  match.value = await updateMatchArmyList(match.value.id, code, armyId)
}

async function persistPlayer2ArmyList(code: string, armyId?: number | null) {
  if (!match.value) return
  match.value = await updateMatchArmyList(match.value.id, code, armyId)
}

async function onDelete() {
  if (!match.value || !isAdmin.value) return
  const inProgress = match.value.status === 'in_progress'
  const deleteMessage = inProgress
    ? 'Supprimer cette partie en cours ?'
    : showsElo.value
      ? 'Supprimer ce match ? Les classements ELO et victoires/défaites seront annulés.'
      : 'Supprimer ce match amical ?'
  if (!window.confirm(deleteMessage)) {
    return
  }

  deleting.value = true
  try {
    await deleteMatch(match.value.id)
    toast.success('Match supprimé')
    router.push('/matchs')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Suppression impossible')
  } finally {
    deleting.value = false
  }
}

onMounted(loadMatch)
</script>

<template>
  <div class="page-stack">
    <PageTitleTabs
      :tabs="matchsTabs"
      ariaLabel="Sections des matchs"
      :current="{ label: `Match #${matchId}` }"
    />

    <section class="page-header">
      <div class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <p v-if="match" class="page-description">{{ matchupLabel }}</p>
        <div class="flex flex-wrap items-center gap-2 sm:ml-auto">
          <Button
            v-if="canResume"
            type="button"
            size="sm"
            @click="resumePartie"
          >
            <Play class="size-4" />
            Reprendre
          </Button>
          <Button
            v-if="isAdmin && match"
            type="button"
            variant="destructive"
            size="sm"
            :disabled="deleting"
            @click="onDelete"
          >
            <Trash2 class="size-4" />
            {{
              deleting
                ? 'Suppression…'
                : match.status === 'in_progress'
                  ? 'Supprimer la partie'
                  : 'Supprimer le match'
            }}
          </Button>
        </div>
      </div>
    </section>

    <Alert v-if="!apiOnline && !loading" variant="destructive" class="neon-panel-accent">
      <AlertTitle>Match introuvable</AlertTitle>
      <AlertDescription>
        Ce match n'existe pas ou l'API est indisponible.
      </AlertDescription>
    </Alert>

    <div v-else-if="loading" class="text-sm text-muted-foreground">
      Chargement…
    </div>

    <div v-else-if="match" class="page-panel-scroll min-h-0 flex-1 space-y-4 overflow-y-auto">
      <Card class="neon-panel">
        <CardHeader>
          <CardTitle class="flex items-center gap-2">
            <Swords class="size-5 text-primary" />
            Résultat
          </CardTitle>
          <CardDescription class="flex flex-wrap items-center gap-x-1.5 gap-y-1">
            <span>{{ matchDateLabel }}</span>
            <template v-if="match.tournament_id != null">
              <span aria-hidden="true">·</span>
              <RouterLink
                :to="{ name: 'tournoi', params: { id: match.tournament_id } }"
                class="font-medium text-primary hover:underline"
              >
                {{ match.tournament_name ?? `Tournoi #${match.tournament_id}` }}
              </RouterLink>
              <template v-if="tournamentPhaseText">
                <span>—</span>
                <span>{{ tournamentPhaseText }}</span>
              </template>
            </template>
            <template v-else>
              <span aria-hidden="true">·</span>
              <span>{{ casualContextLabel }}</span>
            </template>
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div
            class="grid grid-cols-1 items-stretch gap-3 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.2fr)_minmax(0,0.9fr)]"
          >
            <!-- Joueur 1 -->
            <section
              class="player-match-panel player-match-panel--compact space-y-2.5"
              :class="{ 'player-match-panel--winner': isWinner('player1') }"
            >
              <div class="flex items-center gap-3">
                <ArmyLogo :army-id="match.player1_army_id" size="lg" />
                <PlayerLink
                  :name="match.player1"
                  :display-name="match.player1_display_name"
                  class="text-lg font-semibold"
                />
              </div>
              <p
                v-if="armyListHidden(match.player1_army_list_code)"
                class="text-sm text-muted-foreground italic"
              >
                Liste secrète jusqu’à la fin du tournoi
              </p>
              <MatchArmyListCard
                v-else
                :code="match.player1_army_list_code"
                :current-army-id="match.player1_army_id"
                :can-edit="canEditArmyList(match.player1)"
                :persist="persistPlayer1ArmyList"
              />
              <CombatEspritPlayerHand
                v-if="isCombatEspritMatch"
                compact
                :ban="resolveSecondary(match.player1_chosen_secondary)"
                :picks="handPicks(match.player1_secondary_slugs)"
                :badges="player1HandBadges"
              />
              <ContentHoverTip
                v-else-if="match.player1_chosen_secondary"
                class="combat-esprit-hand-slot combat-esprit-hand-slot--filled mt-2 block w-1/2 max-w-28"
                :title="secondaryLabel(match.player1_chosen_secondary)"
                :body-md="secondaryBody(match.player1_chosen_secondary)"
              >
                <img
                  v-if="secondaryImageSrc(match.player1_chosen_secondary)"
                  :src="secondaryImageSrc(match.player1_chosen_secondary)"
                  :alt="secondaryLabel(match.player1_chosen_secondary)"
                  class="combat-esprit-hand-slot-image"
                />
                <span v-else class="combat-esprit-hand-slot-fallback">{{
                  secondaryLabel(match.player1_chosen_secondary)
                }}</span>
              </ContentHoverTip>
              <div
                v-if="objectivesPoints('player1') != null"
                class="space-y-0.5 text-sm"
              >
                <p>
                  Points d'objectifs :
                  <span class="font-medium tabular-nums">{{
                    objectivesPoints('player1')
                  }}</span>
                </p>
                <p>
                  Points de survivants :
                  <span class="font-medium tabular-nums">{{
                    survivorsPoints('player1')
                  }}</span>
                </p>
              </div>
              <p
                v-if="match.status !== 'in_progress' && showsElo"
                class="text-xs text-muted-foreground"
              >
                ELO {{ Math.round(match.player1_old) }}
                → {{ Math.round(match.player1_new) }}
                <span class="font-medium text-primary">
                  ({{ eloDelta(match.player1_old, match.player1_new) }})
                </span>
              </p>
            </section>

            <!-- Partie (centre) -->
            <section class="player-match-panel space-y-3 text-sm">
              <p>
                <span class="text-muted-foreground">Scénario :</span>
                <RouterLink
                  v-if="packScenario?.slug"
                  :to="{ name: 'scenarios', query: { scenario: packScenario.slug } }"
                  class="ml-1 font-medium text-primary hover:underline"
                >
                  {{ scenarioLabel }}
                </RouterLink>
                <a
                  v-else-if="scenarioLabel && match.scenario_url"
                  :href="externalHref(match.scenario_url)"
                  target="_blank"
                  rel="noopener noreferrer"
                  class="ml-1 font-medium text-primary hover:underline"
                >
                  {{ scenarioLabel }}
                </a>
                <span v-else-if="scenarioLabel" class="ml-1 font-medium">
                  {{ scenarioLabel }}
                </span>
                <span v-else class="ml-1 italic text-muted-foreground">non renseigné</span>
              </p>

              <div v-if="lieutenantLines" class="space-y-0.5">
                <p v-for="(line, index) in lieutenantLines" :key="index">
                  {{ line }}
                </p>
              </div>
              <p v-else class="italic text-muted-foreground">
                Jet de lieutenant non renseigné
              </p>

              <p v-if="outcomeLine" class="font-display text-base font-semibold text-primary">
                {{ outcomeLine }}
              </p>
            </section>

            <!-- Joueur 2 -->
            <section
              class="player-match-panel player-match-panel--compact space-y-2.5"
              :class="{ 'player-match-panel--winner': isWinner('player2') }"
            >
              <div class="flex items-center gap-3">
                <ArmyLogo :army-id="match.player2_army_id" size="lg" />
                <PlayerLink
                  :name="match.player2"
                  :display-name="match.player2_display_name"
                  class="text-lg font-semibold"
                />
              </div>
              <p
                v-if="armyListHidden(match.player2_army_list_code)"
                class="text-sm text-muted-foreground italic"
              >
                Liste secrète jusqu’à la fin du tournoi
              </p>
              <MatchArmyListCard
                v-else
                :code="match.player2_army_list_code"
                :current-army-id="match.player2_army_id"
                :can-edit="canEditArmyList(match.player2)"
                :persist="persistPlayer2ArmyList"
              />
              <CombatEspritPlayerHand
                v-if="isCombatEspritMatch"
                compact
                :ban="resolveSecondary(match.player2_chosen_secondary)"
                :picks="handPicks(match.player2_secondary_slugs)"
                :badges="player2HandBadges"
              />
              <ContentHoverTip
                v-else-if="match.player2_chosen_secondary"
                class="combat-esprit-hand-slot combat-esprit-hand-slot--filled mt-2 ml-auto block w-1/2 max-w-28"
                :title="secondaryLabel(match.player2_chosen_secondary)"
                :body-md="secondaryBody(match.player2_chosen_secondary)"
              >
                <img
                  v-if="secondaryImageSrc(match.player2_chosen_secondary)"
                  :src="secondaryImageSrc(match.player2_chosen_secondary)"
                  :alt="secondaryLabel(match.player2_chosen_secondary)"
                  class="combat-esprit-hand-slot-image"
                />
                <span v-else class="combat-esprit-hand-slot-fallback">{{
                  secondaryLabel(match.player2_chosen_secondary)
                }}</span>
              </ContentHoverTip>
              <div
                v-if="objectivesPoints('player2') != null"
                class="space-y-0.5 text-sm"
              >
                <p>
                  Points d'objectifs :
                  <span class="font-medium tabular-nums">{{
                    objectivesPoints('player2')
                  }}</span>
                </p>
                <p>
                  Points de survivants :
                  <span class="font-medium tabular-nums">{{
                    survivorsPoints('player2')
                  }}</span>
                </p>
              </div>
              <p
                v-if="match.status !== 'in_progress' && showsElo"
                class="text-xs text-muted-foreground"
              >
                ELO {{ Math.round(match.player2_old) }}
                → {{ Math.round(match.player2_new) }}
                <span class="font-medium text-primary">
                  ({{ eloDelta(match.player2_old, match.player2_new) }})
                </span>
              </p>
            </section>
          </div>
        </CardContent>
      </Card>

      <div class="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Card class="neon-panel relative">
          <CardHeader>
            <CardTitle class="text-base">
              CR — {{ match.player1_display_name ?? match.player1 }}
            </CardTitle>
            <CardDescription v-if="match.player1_report">
              {{
                reportDateLabel(
                  match.player1_report.created_at,
                  match.player1_report.updated_at,
                )
              }}
            </CardDescription>
          </CardHeader>
          <CardContent>
            <AdminContentEditor
              :can-edit="canEditReport(match.player1)"
              :body="match.player1_report?.body_md ?? ''"
              :rows="12"
              simple-markdown
              :persist="persistPlayer1Report"
            >
              <MarkdownContent
                v-if="match.player1_report?.body_md?.trim()"
                :source="match.player1_report.body_md"
              />
              <p v-else class="text-sm text-muted-foreground italic">
                Aucun compte rendu pour l'instant.
              </p>
            </AdminContentEditor>
          </CardContent>
        </Card>

        <Card class="neon-panel relative">
          <CardHeader>
            <CardTitle class="text-base">
              CR — {{ match.player2_display_name ?? match.player2 }}
            </CardTitle>
            <CardDescription v-if="match.player2_report">
              {{
                reportDateLabel(
                  match.player2_report.created_at,
                  match.player2_report.updated_at,
                )
              }}
            </CardDescription>
          </CardHeader>
          <CardContent>
            <AdminContentEditor
              :can-edit="canEditReport(match.player2)"
              :body="match.player2_report?.body_md ?? ''"
              :rows="12"
              simple-markdown
              :persist="persistPlayer2Report"
            >
              <MarkdownContent
                v-if="match.player2_report?.body_md?.trim()"
                :source="match.player2_report.body_md"
              />
              <p v-else class="text-sm text-muted-foreground italic">
                Aucun compte rendu pour l'instant.
              </p>
            </AdminContentEditor>
          </CardContent>
        </Card>
      </div>

      <p v-if="!isAuthenticated" class="text-sm text-muted-foreground">
        Connectez-vous avec Discord pour saisir votre liste ou rédiger votre compte rendu si vous
        avez participé à ce match.
        <Button type="button" size="sm" variant="link" class="px-1" @click="login">
          Connexion
        </Button>
      </p>
    </div>
  </div>
</template>
