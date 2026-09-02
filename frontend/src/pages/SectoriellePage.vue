<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useTitle } from '@vueuse/core'
import { toast } from 'vue-sonner'
import {
  fetchArmyMatches,
  fetchArmyPlayers,
  fetchArmyStats,
  fetchPrefs,
  updatePrefs,
  type ArmySortMode,
} from '@/lib/api'
import { pageTitle } from '@/lib/pageTitle'
import { classementTabs } from '@/lib/pageTitleTabs'
import type { ArmyPlayerStats, MatchRecord, RankedArmy } from '@/types/elo'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import WinDrawLossBar from '@/components/WinDrawLossBar.vue'
import ArmyLogo from '@/components/ArmyLogo.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import RecentMatchesList from '@/components/RecentMatchesList.vue'
import { useArmies } from '@/composables/useArmies'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
} from '@/components/ui/card'

const route = useRoute()
const router = useRouter()
const { ensureLoaded, getArmy } = useArmies()

const army = ref<RankedArmy | null>(null)
const matches = ref<MatchRecord[]>([])
const players = ref<ArmyPlayerStats[]>([])
const loading = ref(true)
const loadingMatches = ref(true)
const matchesPage = ref(1)
const sortMode = ref<ArmySortMode>('win_rate')

const MATCHES_PAGE_SIZE = 5

const armyId = computed(() => Number(route.params.id))

const armyName = computed(
  () => getArmy(armyId.value)?.name ?? `Sectorielle #${armyId.value}`,
)

const armyMatchesCount = computed(() => {
  if (!army.value) return 0
  return army.value.wins + army.value.draws + army.value.losses
})

const title = useTitle()

watch(
  armyName,
  (name) => {
    title.value = pageTitle(name)
  },
  { immediate: true },
)

function formatWinRate(winRate: number) {
  return `${winRate.toLocaleString('fr-FR', {
    minimumFractionDigits: 0,
    maximumFractionDigits: 1,
  })} %`
}

function plural(count: number, singular: string, pluralLabel: string) {
  return count > 1 ? pluralLabel : singular
}

function totalMatches(entry: ArmyPlayerStats) {
  return entry.wins + entry.draws + entry.losses
}

function otherSortMode(mode: ArmySortMode): ArmySortMode {
  return mode === 'win_rate' ? 'matches' : 'win_rate'
}

const sortedPlayers = computed(() => {
  const entries = [...players.value]

  if (sortMode.value === 'win_rate') {
    entries.sort(
      (a, b) =>
        b.win_rate - a.win_rate ||
        totalMatches(b) - totalMatches(a) ||
        a.display_name.localeCompare(b.display_name, 'fr', { sensitivity: 'base' }),
    )
  } else {
    entries.sort(
      (a, b) =>
        totalMatches(b) - totalMatches(a) ||
        b.win_rate - a.win_rate ||
        a.display_name.localeCompare(b.display_name, 'fr', { sensitivity: 'base' }),
    )
  }

  return entries
})

function playerRecordLabel(entry: ArmyPlayerStats) {
  const victoires = `${entry.wins} ${plural(entry.wins, 'victoire', 'victoires')}`
  const nuls = `${entry.draws} ${plural(entry.draws, 'nul', 'nuls')}`
  const defaites = `${entry.losses} ${plural(entry.losses, 'défaite', 'défaites')}`
  return `Win rate : ${formatWinRate(entry.win_rate)} · ${victoires}, ${nuls}, ${defaites}`
}

function sortButtonClass(mode: ArmySortMode) {
  return sortMode.value === mode
    ? 'border-primary bg-primary! text-primary-foreground hover:bg-primary/90'
    : 'border-border bg-black text-white hover:text-primary'
}

function setSortMode(mode: ArmySortMode) {
  const next = sortMode.value === mode ? otherSortMode(mode) : mode
  sortMode.value = next
  void updatePrefs({ army_sort_mode: next }).catch(() => {
    // Keep the local choice even if persistence fails.
  })
}

function goToMatchesPage(nextPage: number) {
  matchesPage.value = nextPage
}

async function loadArmy() {
  const id = armyId.value
  if (!Number.isFinite(id) || id <= 0) {
    army.value = null
    players.value = []
    return
  }

  loading.value = true
  try {
    await ensureLoaded()
    const [stats, armyPlayers] = await Promise.all([
      fetchArmyStats(id),
      fetchArmyPlayers(id),
    ])
    army.value = stats
    players.value = armyPlayers
  } catch (error) {
    army.value = null
    players.value = []
    toast.error(error instanceof Error ? error.message : 'Sectorielle introuvable')
    router.push('/sectorielles')
  } finally {
    loading.value = false
  }
}

async function loadMatches() {
  const id = armyId.value
  if (!Number.isFinite(id) || id <= 0) {
    matches.value = []
    matchesPage.value = 1
    return
  }

  loadingMatches.value = true
  try {
    matches.value = await fetchArmyMatches(id)
    matchesPage.value = 1
  } catch {
    matches.value = []
    matchesPage.value = 1
  } finally {
    loadingMatches.value = false
  }
}

async function refresh() {
  await Promise.all([loadArmy(), loadMatches()])
}

watch(armyId, refresh, { immediate: true })
onMounted(() => {
  void fetchPrefs()
    .then((prefs) => {
      if (prefs.army_sort_mode === 'win_rate' || prefs.army_sort_mode === 'matches') {
        sortMode.value = prefs.army_sort_mode
      }
    })
    .catch(() => {
      // Keep the default sort if prefs cannot be loaded.
    })
})
</script>

<template>
  <div class="page-stack">
    <div
      v-if="loading"
      class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
    >
      Chargement de la sectorielle...
    </div>

    <template v-else-if="army">
      <PageTitleTabs
        :tabs="classementTabs"
        ariaLabel="Sections du classement"
        :current="{ label: armyName }"
      />

      <div class="grid shrink-0 gap-3 lg:grid-cols-2">
        <Card size="sm" class="neon-panel">
          <CardContent class="flex h-full flex-col justify-center gap-3 py-2">
            <div class="flex items-center gap-3">
              <ArmyLogo :army-id="army.army_id" class="!size-12" />
              <div class="min-w-0">
                <h1
                  class="truncate text-lg font-semibold leading-tight"
                  :title="armyName"
                >
                  {{ armyName }}
                </h1>
                <p class="truncate text-xs text-muted-foreground">
                  Rang #{{ army.rank }}
                </p>
              </div>
            </div>

            <dl class="grid grid-cols-5 gap-2">
              <div class="rounded border px-2 py-1.5">
                <dt class="text-[11px] text-muted-foreground">Win rate</dt>
                <dd class="elo-score font-display text-base font-semibold">
                  {{ formatWinRate(army.win_rate) }}
                </dd>
              </div>
              <div class="rounded border px-2 py-1.5">
                <dt class="text-[11px] text-muted-foreground">Parties</dt>
                <dd class="font-display text-base font-semibold text-primary">
                  {{ armyMatchesCount }}
                </dd>
              </div>
              <div class="rounded border px-2 py-1.5">
                <dt class="text-[11px] text-muted-foreground">Victoires</dt>
                <dd class="font-display text-base font-semibold">
                  {{ army.wins }}
                </dd>
              </div>
              <div class="rounded border px-2 py-1.5">
                <dt class="text-[11px] text-muted-foreground">Nuls</dt>
                <dd class="font-display text-base font-semibold">
                  {{ army.draws }}
                </dd>
              </div>
              <div class="rounded border px-2 py-1.5">
                <dt class="text-[11px] text-muted-foreground">Défaites</dt>
                <dd class="font-display text-base font-semibold">
                  {{ army.losses }}
                </dd>
              </div>
            </dl>

            <WinDrawLossBar
              bar-only
              :wins="army.wins"
              :draws="army.draws"
              :losses="army.losses"
            />
          </CardContent>
        </Card>

        <div class="lg:relative lg:min-h-0">
          <Card
            size="sm"
            class="neon-panel flex min-h-0 flex-col lg:absolute lg:inset-0"
          >
            <CardContent class="flex min-h-0 flex-1 flex-col gap-1.5 overflow-hidden py-2">
              <div class="flex shrink-0 flex-wrap items-center justify-between gap-2">
                <p class="text-sm font-semibold leading-none">
                  Joueurs
                </p>
                <div class="flex items-center gap-2">
                  <span class="text-xs font-medium text-muted-foreground">Tri :</span>
                  <div class="flex items-center gap-0">
                    <Button
                      type="button"
                      size="xs"
                      variant="outline"
                      :class="['rounded-r-none', sortButtonClass('win_rate')]"
                      @click="setSortMode('win_rate')"
                    >
                      Win rate
                    </Button>
                    <Button
                      type="button"
                      size="xs"
                      variant="outline"
                      :class="[
                        'rounded-l-none border-l-0',
                        sortButtonClass('matches'),
                      ]"
                      @click="setSortMode('matches')"
                    >
                      Nb parties
                    </Button>
                  </div>
                </div>
              </div>
              <ul
                v-if="sortedPlayers.length > 0"
                class="min-h-0 flex-1 space-y-1 overflow-y-auto"
              >
                <li
                  v-for="entry in sortedPlayers"
                  :key="entry.player_name"
                  class="flex min-w-0 items-center gap-2 overflow-hidden rounded border px-2 py-1.5 text-xs"
                >
                  <span class="min-w-0 shrink-0 truncate font-medium">
                    <PlayerLink
                      :name="entry.player_name"
                      :display-name="entry.display_name"
                    />
                  </span>
                  <span
                    class="shrink-0 tabular-nums text-muted-foreground"
                    :title="`${totalMatches(entry)} parties`"
                  >
                    {{ totalMatches(entry) }}
                  </span>
                  <span
                    class="min-w-0 flex-1 truncate tabular-nums text-muted-foreground"
                    :title="playerRecordLabel(entry)"
                  >
                    {{ playerRecordLabel(entry) }}
                  </span>
                  <div class="w-24 shrink-0 sm:w-32">
                    <WinDrawLossBar
                      bar-only
                      :wins="entry.wins"
                      :draws="entry.draws"
                      :losses="entry.losses"
                    />
                  </div>
                </li>
              </ul>
              <p
                v-else
                class="rounded border border-dashed px-3 py-4 text-center text-xs text-muted-foreground"
              >
                Aucun joueur n'a encore joué cette sectorielle.
              </p>
            </CardContent>
          </Card>
        </div>
      </div>

      <RecentMatchesList
        :matches="matches"
        :loading="loadingMatches"
        :page="matchesPage"
        :page-size="MATCHES_PAGE_SIZE"
        :perspective-army-id="army.army_id"
        title="Historique des parties"
        description="Toutes les parties enregistrées avec cette sectorielle."
        empty-message="Aucune partie enregistrée pour cette sectorielle."
        client-side
        @page-change="goToMatchesPage"
      />
    </template>
  </div>
</template>
