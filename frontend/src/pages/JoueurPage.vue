<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter, RouterLink } from 'vue-router'
import { useTitle } from '@vueuse/core'
import { toast } from 'vue-sonner'
import {
  fetchPlayer,
  fetchPlayerArmies,
  fetchPlayerMatches,
  fetchPlayerTournaments,
  fetchRanking,
  updateProfile,
} from '@/lib/api'
import { pageTitle } from '@/lib/pageTitle'
import { classementTabs } from '@/lib/pageTitleTabs'
import { useAuth } from '@/composables/useAuth'
import { useAppSidePanel } from '@/composables/useAppSidePanel'
import type {
  MatchRecord,
  PlayerArmyStats,
  PlayerProfile,
  PlayerTournamentResult,
  RankedPlayer,
} from '@/types/elo'
import RecentMatchesList from '@/components/RecentMatchesList.vue'
import PlayerArmyStatsTable from '@/components/PlayerArmyStatsTable.vue'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import WinDrawLossBar from '@/components/WinDrawLossBar.vue'
import ArmyLogo from '@/components/ArmyLogo.vue'
import { useArmies } from '@/composables/useArmies'
import PlayerPreferencesForm from '@/components/PlayerPreferencesForm.vue'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

const route = useRoute()
const router = useRouter()
const { user, refresh: refreshAuth } = useAuth()
const { setCustomSide } = useAppSidePanel()
const { ensureLoaded: ensureArmiesLoaded, getArmy } = useArmies()

const player = ref<RankedPlayer | null>(null)
const profile = ref<PlayerProfile | null>(null)
const matches = ref<MatchRecord[]>([])
const armyStats = ref<PlayerArmyStats[]>([])
const tournamentResults = ref<PlayerTournamentResult[]>([])
const loading = ref(true)
const loadingMatches = ref(true)
const loadingArmyStats = ref(true)
const savingProfile = ref(false)
const matchesPage = ref(1)

const MATCHES_PAGE_SIZE = 5

const localDisplayName = ref('')
const localAvatarUrl = ref('')

const playerName = computed(() => String(route.params.name ?? ''))

const title = useTitle()

const headerTitle = computed(
  () =>
    profile.value?.display_name ??
    player.value?.display_name ??
    player.value?.name ??
    '',
)

const playerMatchesCount = computed(() => {
  if (!player.value) return 0
  return player.value.wins + player.value.draws + player.value.losses
})

const playerWinRate = computed(() => {
  if (!player.value) return 0
  const total = playerMatchesCount.value
  if (total === 0) return 0
  const effectiveWins = player.value.wins + player.value.draws * 0.5
  return (effectiveWins / total) * 100
})

function formatWinRate(winRate: number) {
  return `${winRate.toLocaleString('fr-FR', {
    minimumFractionDigits: 0,
    maximumFractionDigits: 1,
  })} %`
}

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

const isOwnProfile = computed(() => Boolean(profile.value?.is_own_profile))

watch(
  isOwnProfile,
  (own) => {
    setCustomSide(own)
  },
  { immediate: true },
)

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

function goToMatchesPage(nextPage: number) {
  matchesPage.value = nextPage
}

async function loadArmyStats() {
  const name = playerName.value
  if (!name) {
    armyStats.value = []
    loadingArmyStats.value = false
    return
  }

  loadingArmyStats.value = true
  try {
    armyStats.value = await fetchPlayerArmies(name)
  } catch {
    armyStats.value = []
  } finally {
    loadingArmyStats.value = false
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
  await Promise.all([loadPlayer(), loadMatches(), loadArmyStats(), loadTournaments()])
}

watch(playerName, refresh, { immediate: true })
onMounted(refresh)
</script>

<template>
  <div class="page-stack min-h-0 flex-1 gap-3 overflow-y-auto">
    <div
      v-if="loading"
      class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
    >
      Chargement du joueur...
    </div>

    <template v-else-if="player">
      <div class="sticky top-0 z-20 shrink-0 bg-background/95 backdrop-blur-md">
        <PageTitleTabs
          :tabs="classementTabs"
          ariaLabel="Sections du classement"
          :current="{ label: headerTitle || playerName }"
        />
      </div>

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

      <Card v-if="isOwnProfile" class="neon-panel shrink-0 lg:hidden">
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

      <div class="grid shrink-0 gap-3 lg:grid-cols-2">
          <Card size="sm" class="neon-panel">
            <CardContent class="flex h-full flex-col justify-center gap-3 py-2">
              <div class="grid grid-cols-4 items-stretch gap-2">
                <div class="col-span-2 flex min-w-0 items-center gap-3">
                  <img
                    v-if="profile?.avatar_url"
                    :src="profile.avatar_url"
                    :alt="headerTitle"
                    class="size-12 shrink-0 rounded-full border border-primary/30 object-cover"
                  />
                  <div class="min-w-0">
                    <h1
                      class="truncate text-lg font-semibold leading-tight"
                      :title="headerTitle"
                    >
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
                <div class="rounded border px-2 py-1.5">
                  <dt class="text-[11px] text-muted-foreground">Rang</dt>
                  <dd class="font-display text-base font-semibold text-primary tabular-nums">
                    #{{ player.rank }}
                  </dd>
                </div>
                <div class="rounded border px-2 py-1.5">
                  <dt class="text-[11px] text-muted-foreground">ELO</dt>
                  <dd class="elo-score font-display text-base font-semibold tabular-nums">
                    {{ Math.round(player.rating) }}
                  </dd>
                </div>
              </div>

              <dl class="grid grid-cols-5 gap-2">
                <div class="rounded border px-2 py-1.5">
                  <dt class="text-[11px] text-muted-foreground">Win rate</dt>
                  <dd class="elo-score font-display text-base font-semibold">
                    {{ formatWinRate(playerWinRate) }}
                  </dd>
                </div>
                <div class="rounded border px-2 py-1.5">
                  <dt class="text-[11px] text-muted-foreground">Parties</dt>
                  <dd class="font-display text-base font-semibold text-primary">
                    {{ playerMatchesCount }}
                  </dd>
                </div>
                <div class="rounded border px-2 py-1.5">
                  <dt class="text-[11px] text-muted-foreground">Victoires</dt>
                  <dd class="font-display text-base font-semibold">
                    {{ player.wins }}
                  </dd>
                </div>
                <div class="rounded border px-2 py-1.5">
                  <dt class="text-[11px] text-muted-foreground">Nuls</dt>
                  <dd class="font-display text-base font-semibold">
                    {{ player.draws }}
                  </dd>
                </div>
                <div class="rounded border px-2 py-1.5">
                  <dt class="text-[11px] text-muted-foreground">Défaites</dt>
                  <dd class="font-display text-base font-semibold">
                    {{ player.losses }}
                  </dd>
                </div>
              </dl>

              <WinDrawLossBar
                bar-only
                :wins="player.wins"
                :draws="player.draws"
                :losses="player.losses"
              />
            </CardContent>
          </Card>

          <div class="lg:relative lg:min-h-0">
            <Card
              size="sm"
              class="neon-panel flex min-h-0 flex-col lg:absolute lg:inset-0"
            >
              <CardContent class="flex min-h-0 flex-1 flex-col gap-1.5 overflow-hidden py-2">
                <p class="shrink-0 text-sm font-semibold leading-none">
                  Tournois
                </p>
                <ul
                  v-if="tournamentResults.length > 0"
                  class="min-h-0 flex-1 space-y-1 overflow-y-auto"
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
        </div>

        <PlayerArmyStatsTable
          :stats="armyStats"
          :loading="loadingArmyStats"
        />

        <Card size="sm" class="neon-panel shrink-0">
          <CardHeader class="pb-0">
            <CardTitle>Historique des parties</CardTitle>
          </CardHeader>
          <CardContent>
            <RecentMatchesList
              v-if="player"
              :matches="matches"
              :loading="loadingMatches"
              :page="matchesPage"
              :page-size="MATCHES_PAGE_SIZE"
              :perspective-player="player.name"
              show-elo
              bare
              client-side
              empty-message="Aucune partie enregistrée pour ce joueur."
              @page-change="goToMatchesPage"
            />
          </CardContent>
        </Card>
    </template>
  </div>
</template>
