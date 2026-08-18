<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter, RouterLink } from 'vue-router'
import { useTitle } from '@vueuse/core'
import { ChevronLeft, ChevronRight, Eye } from '@lucide/vue'
import { toast } from 'vue-sonner'
import {
  fetchPlayer,
  fetchPlayerMatches,
  fetchPlayerTournaments,
  fetchRanking,
  updateProfile,
} from '@/lib/api'
import { pageTitle } from '@/lib/pageTitle'
import { formatMatchRecordedDate } from '@/lib/tournamentMatchDisplay'
import { matchCountsForElo } from '@/lib/matchElo'
import { classementTabs } from '@/lib/pageTitleTabs'
import { useAuth } from '@/composables/useAuth'
import { useAppSidePanel } from '@/composables/useAppSidePanel'
import type {
  MatchOutcome,
  MatchRecord,
  PlayerProfile,
  PlayerTournamentResult,
  RankedPlayer,
} from '@/types/elo'
import MatchResultBadges from '@/components/MatchResultBadges.vue'
import MatchContextCell from '@/components/MatchContextCell.vue'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import WinDrawLossBar from '@/components/WinDrawLossBar.vue'
import ArmyLogo from '@/components/ArmyLogo.vue'
import { useArmies } from '@/composables/useArmies'
import PlayerLink from '@/components/PlayerLink.vue'
import PlayerPreferencesForm from '@/components/PlayerPreferencesForm.vue'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'

const route = useRoute()
const router = useRouter()
const { user, refresh: refreshAuth } = useAuth()
const { setCustomSide } = useAppSidePanel()
const { ensureLoaded: ensureArmiesLoaded, getArmy } = useArmies()

const player = ref<RankedPlayer | null>(null)
const profile = ref<PlayerProfile | null>(null)
const matches = ref<MatchRecord[]>([])
const tournamentResults = ref<PlayerTournamentResult[]>([])
const loading = ref(true)
const loadingMatches = ref(true)
const savingProfile = ref(false)

const localDisplayName = ref('')
const localAvatarUrl = ref('')
const matchesPage = ref(1)

const MATCHES_PAGE_SIZE = 5

const playerName = computed(() => String(route.params.name ?? ''))

const title = useTitle()

const headerTitle = computed(
  () =>
    profile.value?.display_name ??
    player.value?.display_name ??
    player.value?.name ??
    '',
)

watch(
  () => headerTitle.value || playerName.value,
  (name) => {
    if (name) title.value = pageTitle(name)
  },
  { immediate: true },
)

function normalize(name: string) {
  return name.trim().toLowerCase()
}

function isSamePlayer(a: string, b: string) {
  return normalize(a) === normalize(b)
}

function flipOutcome(outcome: MatchOutcome | null | undefined): MatchOutcome {
  if (!outcome || outcome === 'draw') return 'draw'
  if (outcome === 'player1_win') return 'player2_win'
  return 'player1_win'
}

function normalizeMatchForPlayer(match: MatchRecord, name: string): MatchRecord {
  if (isSamePlayer(match.player1, name)) {
    return match
  }

  return {
    ...match,
    player1: match.player2,
    player2: match.player1,
    player1_display_name: match.player2_display_name,
    player2_display_name: match.player1_display_name,
    player1_old: match.player2_old,
    player1_new: match.player2_new,
    player2_old: match.player1_old,
    player2_new: match.player1_new,
    player1_objectives: match.player2_objectives,
    player1_survivors: match.player2_survivors,
    player2_objectives: match.player1_objectives,
    player2_survivors: match.player1_survivors,
    player1_army_id: match.player2_army_id,
    player2_army_id: match.player1_army_id,
    outcome: flipOutcome(match.outcome),
  }
}

const matchRows = computed(() => {
  if (!player.value) return []

  return matches.value
    .filter((match) => match.status !== 'in_progress')
    .map((match) => {
      const normalized = normalizeMatchForPlayer(match, player.value!.name)
      return {
        id: match.id,
        date: formatMatchRecordedDate(match.recorded_at) ?? '—',
        normalized,
      }
    })
})

const isOwnProfile = computed(() => Boolean(profile.value?.is_own_profile))

watch(
  isOwnProfile,
  (own) => {
    setCustomSide(own)
  },
  { immediate: true },
)

const matchesTotalPages = computed(() =>
  Math.max(1, Math.ceil(matchRows.value.length / MATCHES_PAGE_SIZE)),
)

const pagedMatchRows = computed(() => {
  const start = (matchesPage.value - 1) * MATCHES_PAGE_SIZE
  return matchRows.value.slice(start, start + MATCHES_PAGE_SIZE)
})

const matchesPageStart = computed(() => {
  if (matchRows.value.length === 0) return 0
  return (matchesPage.value - 1) * MATCHES_PAGE_SIZE + 1
})

const matchesPageEnd = computed(() =>
  Math.min(matchesPage.value * MATCHES_PAGE_SIZE, matchRows.value.length),
)

function goToMatchesPage(nextPage: number) {
  if (nextPage < 1 || nextPage > matchesTotalPages.value) return
  matchesPage.value = nextPage
}

function syncProfileForm() {
  localDisplayName.value = user.value?.local_display_name ?? ''
  localAvatarUrl.value = user.value?.local_avatar_url ?? ''
}

async function loadPlayer() {
  const name = playerName.value
  if (!name) {
    player.value = null
    profile.value = null
    return
  }

  loading.value = true
  try {
    const [ranking, playerData] = await Promise.all([
      fetchRanking(),
      fetchPlayer(name),
    ])

    profile.value = playerData

    const ranked = ranking.find(
      (entry) => normalize(entry.name) === normalize(name),
    )

    player.value = ranked ?? {
      ...playerData,
      display_name: playerData.display_name,
      rank: ranking.length + 1,
      top_armies: [],
    }

    if (playerData.is_own_profile) {
      await refreshAuth()
      syncProfileForm()
    }
  } catch (error) {
    player.value = null
    profile.value = null
    toast.error(error instanceof Error ? error.message : 'Joueur introuvable')
    router.push('/classement')
  } finally {
    loading.value = false
  }
}

async function loadMatches() {
  const name = playerName.value
  if (!name) {
    matches.value = []
    matchesPage.value = 1
    return
  }

  loadingMatches.value = true
  try {
    matches.value = await fetchPlayerMatches(name)
    matchesPage.value = 1
  } catch {
    matches.value = []
    matchesPage.value = 1
  } finally {
    loadingMatches.value = false
  }
}

async function loadTournaments() {
  const name = playerName.value
  if (!name) {
    tournamentResults.value = []
    return
  }
  try {
    tournamentResults.value = await fetchPlayerTournaments(name)
  } catch {
    tournamentResults.value = []
  }
}

async function saveProfile() {
  savingProfile.value = true
  try {
    await updateProfile({
      local_display_name: localDisplayName.value,
      local_avatar_url: localAvatarUrl.value,
    })
    await refreshAuth()
    await loadPlayer()
    toast.success('Profil mis à jour.')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur inconnue')
  } finally {
    savingProfile.value = false
  }
}

async function resetDisplayName() {
  savingProfile.value = true
  try {
    await updateProfile({ clear_local_display_name: true })
    localDisplayName.value = ''
    await refreshAuth()
    await loadPlayer()
    toast.success('Pseudo Discord restauré.')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur inconnue')
  } finally {
    savingProfile.value = false
  }
}

async function resetAvatar() {
  savingProfile.value = true
  try {
    await updateProfile({ clear_local_avatar_url: true })
    localAvatarUrl.value = ''
    await refreshAuth()
    await loadPlayer()
    toast.success('Avatar Discord restauré.')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur inconnue')
  } finally {
    savingProfile.value = false
  }
}

async function refresh() {
  void ensureArmiesLoaded()
  await Promise.all([loadPlayer(), loadMatches(), loadTournaments()])
}

watch(playerName, refresh, { immediate: true })
onMounted(refresh)
</script>

<template>
  <div class="page-stack gap-3">
    <div
      v-if="loading"
      class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
    >
      Chargement du joueur...
    </div>

    <template v-else-if="player">
      <PageTitleTabs
        :tabs="classementTabs"
        aria-label="Sections du classement"
        :current="{ label: headerTitle || playerName }"
      />

      <Teleport defer to="#app-side-panel">
        <Card
          v-if="isOwnProfile"
          class="neon-panel flex h-full min-h-0 flex-col"
        >
          <CardHeader class="shrink-0">
            <CardTitle>Préférences</CardTitle>
            <CardDescription>
              Personnalisez le pseudo et l'avatar affichés à la place de ceux de Discord.
            </CardDescription>
          </CardHeader>
          <CardContent class="min-h-0 flex-1 overflow-y-auto">
            <PlayerPreferencesForm
              v-model:display-name="localDisplayName"
              v-model:avatar-url="localAvatarUrl"
              id-prefix="side"
              :saving="savingProfile"
              @save="saveProfile"
              @reset-display-name="resetDisplayName"
              @reset-avatar="resetAvatar"
            />
          </CardContent>
        </Card>
      </Teleport>

      <Card v-if="isOwnProfile" class="neon-panel lg:hidden">
        <CardHeader>
          <CardTitle>Préférences</CardTitle>
          <CardDescription>
            Personnalisez le pseudo et l'avatar affichés à la place de ceux de Discord.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <PlayerPreferencesForm
            v-model:display-name="localDisplayName"
            v-model:avatar-url="localAvatarUrl"
            id-prefix="mobile"
            :saving="savingProfile"
            @save="saveProfile"
            @reset-display-name="resetDisplayName"
            @reset-avatar="resetAvatar"
          />
        </CardContent>
      </Card>

      <div class="grid gap-3 lg:max-h-44 lg:shrink-0 lg:grid-cols-2">
        <Card size="sm" class="neon-panel min-h-0">
          <CardContent class="flex h-full flex-col justify-center gap-3 py-2">
            <div class="flex items-center gap-3">
              <img
                v-if="profile?.avatar_url"
                :src="profile.avatar_url"
                :alt="headerTitle"
                class="size-12 shrink-0 rounded-full border border-primary/30 object-cover"
              />
              <div class="min-w-0">
                <h1 class="truncate text-lg font-semibold leading-tight">
                  {{ headerTitle }}
                </h1>
                <p
                  v-if="profile?.discord_display_name"
                  class="truncate text-xs text-muted-foreground"
                >
                  {{ profile.discord_display_name }}
                </p>
                <p class="truncate text-xs text-muted-foreground">
                  {{ player.name }}
                </p>
              </div>
            </div>

            <dl class="grid grid-cols-2 gap-2">
              <div class="rounded border px-2.5 py-1.5">
                <dt class="text-[11px] text-muted-foreground">Rang</dt>
                <dd class="font-display text-base font-semibold text-primary">
                  #{{ player.rank }}
                </dd>
              </div>
              <div class="rounded border px-2.5 py-1.5">
                <dt class="text-[11px] text-muted-foreground">ELO</dt>
                <dd class="elo-score font-display text-base font-semibold">
                  {{ Math.round(player.rating) }}
                </dd>
              </div>
            </dl>

            <WinDrawLossBar
              :wins="player.wins"
              :draws="player.draws"
              :losses="player.losses"
              compact
            />
          </CardContent>
        </Card>

        <Card size="sm" class="neon-panel flex min-h-0 flex-col">
          <CardContent class="flex min-h-0 flex-1 flex-col gap-1.5 py-2">
            <p class="shrink-0 text-sm font-semibold leading-none">
              Tournois
            </p>
            <ul
              v-if="tournamentResults.length > 0"
              class="min-h-0 max-h-[9.5rem] space-y-1 overflow-y-auto pr-0.5"
            >
              <li
                v-for="result in tournamentResults"
                :key="result.tournament_id"
                class="flex items-center justify-between gap-2 rounded border px-2 py-1.5 text-xs"
              >
                <div class="flex min-w-0 items-center gap-1.5">
                  <ArmyLogo :army-id="result.army_id ?? undefined" />
                  <div class="min-w-0">
                    <RouterLink
                      :to="{ name: 'tournoi', params: { id: result.tournament_id } }"
                      class="block truncate font-medium text-primary hover:underline"
                    >
                      {{ result.tournament_name }}
                    </RouterLink>
                    <p
                      v-if="result.army_id"
                      class="truncate text-[11px] text-muted-foreground"
                    >
                      {{ getArmy(result.army_id)?.name ?? `Sectorielle #${result.army_id}` }}
                    </p>
                  </div>
                </div>
                <span class="shrink-0 text-xs font-medium">{{ result.placement_label }}</span>
              </li>
            </ul>
            <p
              v-else
              class="rounded border border-dashed px-3 py-4 text-center text-xs text-muted-foreground"
            >
              Aucun tournoi terminé pour ce joueur.
            </p>
          </CardContent>
        </Card>
      </div>

      <Card size="sm" class="neon-panel page-panel-scroll min-h-0 flex-1">
        <CardHeader class="lg:shrink-0 pb-0">
          <CardTitle>Historique des parties</CardTitle>
        </CardHeader>
        <CardContent class="lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
          <div
            v-if="loadingMatches"
            class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
          >
            Chargement des parties...
          </div>

          <div
            v-else-if="matchRows.length === 0"
            class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
          >
            Aucune partie enregistrée pour ce joueur.
          </div>

          <template v-else>
            <Table>
              <TableHeader class="sticky top-0 z-10 bg-card/95 backdrop-blur">
                <TableRow>
                  <TableHead>Date</TableHead>
                  <TableHead class="text-right">Joueur</TableHead>
                  <TableHead class="w-10" aria-hidden="true" />
                  <TableHead>Contexte</TableHead>
                  <TableHead class="text-center">Résultat</TableHead>
                  <TableHead class="w-10" aria-hidden="true" />
                  <TableHead>Adversaire</TableHead>
                  <TableHead class="text-right">ELO</TableHead>
                  <TableHead class="w-12 text-right">
                    <span class="sr-only">Actions</span>
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow v-for="row in pagedMatchRows" :key="row.id">
                  <TableCell class="whitespace-nowrap text-muted-foreground">
                    {{ row.date }}
                  </TableCell>
                  <TableCell class="text-right font-medium">
                    {{ profile?.display_name ?? player.display_name }}
                  </TableCell>
                  <TableCell class="w-10 px-2">
                    <ArmyLogo :army-id="row.normalized.player1_army_id" />
                  </TableCell>
                  <TableCell>
                    <MatchContextCell :match="row.normalized" />
                  </TableCell>
                  <TableCell>
                    <MatchResultBadges :match="row.normalized" />
                  </TableCell>
                  <TableCell class="w-10 px-2">
                    <ArmyLogo :army-id="row.normalized.player2_army_id" />
                  </TableCell>
                  <TableCell>
                    <PlayerLink
                      :name="row.normalized.player2"
                      :display-name="row.normalized.player2_display_name"
                    />
                  </TableCell>
                  <TableCell class="text-right tabular-nums">
                    <template v-if="matchCountsForElo(row.normalized.counts_for_elo)">
                      {{ Math.round(row.normalized.player1_old) }}
                      →
                      <span class="elo-score">{{ Math.round(row.normalized.player1_new) }}</span>
                    </template>
                    <span v-else class="text-muted-foreground">—</span>
                  </TableCell>
                  <TableCell class="text-right">
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      :title="`Voir le match #${row.id}`"
                      :aria-label="`Voir le match #${row.id}`"
                      @click="
                        router.push({ name: 'match', params: { id: String(row.id) } })
                      "
                    >
                      <Eye class="size-4" />
                    </Button>
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>

            <div
              v-if="matchesTotalPages > 1"
              class="mt-3 flex flex-col gap-3 border-t border-border pt-3 sm:flex-row sm:items-center sm:justify-between"
            >
              <p class="text-sm text-muted-foreground">
                {{ matchesPageStart }}–{{ matchesPageEnd }} sur {{ matchRows.length }}
              </p>
              <div class="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  :disabled="matchesPage <= 1 || loadingMatches"
                  @click="goToMatchesPage(matchesPage - 1)"
                >
                  <ChevronLeft class="size-4" />
                  Précédent
                </Button>
                <span class="min-w-24 text-center text-sm text-muted-foreground">
                  Page {{ matchesPage }} / {{ matchesTotalPages }}
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  :disabled="matchesPage >= matchesTotalPages || loadingMatches"
                  @click="goToMatchesPage(matchesPage + 1)"
                >
                  Suivant
                  <ChevronRight class="size-4" />
                </Button>
              </div>
            </div>
          </template>
        </CardContent>
      </Card>
    </template>
  </div>
</template>
