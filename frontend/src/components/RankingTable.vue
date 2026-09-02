<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Podium } from '@lucide/vue'
import type { PlayerArmyUsage, RankedPlayer } from '@/types/elo'
import ArmyLogo from '@/components/ArmyLogo.vue'
import WinDrawLossBar from '@/components/WinDrawLossBar.vue'
import { useArmies } from '@/composables/useArmies'
import {
  fetchPrefs,
  updatePrefs,
  type PlayerSortMode,
} from '@/lib/api'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
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

const props = defineProps<{
  players: RankedPlayer[]
  loading?: boolean
  searchQuery?: string
}>()

const emit = defineEmits<{
  select: [player: RankedPlayer]
}>()

const { ensureLoaded, getArmy } = useArmies()

const PLAYER_SORT_MODES: PlayerSortMode[] = ['elo', 'win_rate', 'matches']

const sortMode = ref<PlayerSortMode>('elo')

onMounted(() => {
  void ensureLoaded()
  void fetchPrefs()
    .then((prefs) => {
      if (PLAYER_SORT_MODES.includes(prefs.player_sort_mode)) {
        sortMode.value = prefs.player_sort_mode
      }
    })
    .catch(() => {
      // Keep the default sort if prefs cannot be loaded.
    })
})

function totalMatches(player: RankedPlayer) {
  return player.wins + player.draws + player.losses
}

function winRate(player: RankedPlayer) {
  const total = totalMatches(player)
  if (total === 0) return 0
  return ((player.wins + player.draws * 0.5) / total) * 100
}

function formatWinRate(value: number) {
  return `${value.toLocaleString('fr-FR', {
    minimumFractionDigits: 0,
    maximumFractionDigits: 1,
  })} %`
}

const rankedActivePlayers = computed(() => {
  const entries = props.players.filter((player) => totalMatches(player) > 0)

  if (sortMode.value === 'elo') {
    entries.sort(
      (a, b) =>
        b.rating - a.rating ||
        winRate(b) - winRate(a) ||
        totalMatches(b) - totalMatches(a) ||
        a.name.localeCompare(b.name, 'fr', { sensitivity: 'base' }),
    )
  } else if (sortMode.value === 'win_rate') {
    entries.sort(
      (a, b) =>
        winRate(b) - winRate(a) ||
        totalMatches(b) - totalMatches(a) ||
        b.rating - a.rating ||
        a.name.localeCompare(b.name, 'fr', { sensitivity: 'base' }),
    )
  } else {
    entries.sort(
      (a, b) =>
        totalMatches(b) - totalMatches(a) ||
        winRate(b) - winRate(a) ||
        b.rating - a.rating ||
        a.name.localeCompare(b.name, 'fr', { sensitivity: 'base' }),
    )
  }

  return entries.map((player, index) => ({ ...player, rank: index + 1 }))
})

const filteredPlayers = computed(() => {
  const query = props.searchQuery?.trim().toLowerCase()
  if (!query) {
    return rankedActivePlayers.value
  }
  return rankedActivePlayers.value.filter((player) =>
    player.display_name.toLowerCase().includes(query)
    || player.name.toLowerCase().includes(query),
  )
})

const isEmpty = computed(() => rankedActivePlayers.value.length === 0)
const hasResults = computed(() => filteredPlayers.value.length > 0)

function rankBadgeClass(rank: number) {
  if (rank === 1) return 'rank-badge-gold tabular-nums font-semibold'
  if (rank === 2) return 'rank-badge-silver tabular-nums font-semibold'
  if (rank === 3) return 'rank-badge-bronze tabular-nums font-semibold'
  return 'rank-badge-outline tabular-nums'
}

function armyTooltip(usage: PlayerArmyUsage) {
  const armyName = getArmy(usage.army_id)?.name ?? 'cette sectorielle'
  const label = usage.matches > 1 ? 'parties' : 'partie'
  return `${usage.matches} ${label} avec ${armyName}`
}

function sortButtonClass(mode: PlayerSortMode) {
  return sortMode.value === mode
    ? 'border-primary bg-primary! text-primary-foreground hover:bg-primary/90'
    : 'border-border bg-black text-white hover:text-primary'
}

function setSortMode(mode: PlayerSortMode) {
  sortMode.value = mode
  void updatePrefs({ player_sort_mode: mode }).catch(() => {
    // Keep the local choice even if persistence fails.
  })
}
</script>

<template>
  <Card class="neon-panel page-panel-scroll">
    <CardHeader class="lg:shrink-0">
      <CardTitle class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
        <div class="flex items-center gap-2">
          <Podium class="size-5 text-primary" />
          Classement ELO
        </div>
        <div
          class="flex w-full flex-col gap-3 sm:w-auto sm:flex-row sm:flex-wrap sm:items-center sm:justify-end"
        >
          <div class="flex items-center gap-2">
            <span class="text-sm font-medium text-muted-foreground">Tri :</span>
            <div class="flex items-center gap-0">
              <Button
                type="button"
                size="xs"
                variant="outline"
                :class="['rounded-r-none', sortButtonClass('elo')]"
                @click="setSortMode('elo')"
              >
                ELO
              </Button>
              <Button
                type="button"
                size="xs"
                variant="outline"
                :class="[
                  'rounded-none border-l-0',
                  sortButtonClass('win_rate'),
                ]"
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
          <div
            v-if="$slots['header-actions']"
            class="flex w-full flex-col gap-2 sm:w-auto sm:flex-row sm:items-center"
          >
            <slot name="header-actions" />
          </div>
        </div>
      </CardTitle>
    </CardHeader>
    <CardContent class="lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
      <div
        v-if="loading"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        Chargement du classement...
      </div>

      <div
        v-else-if="isEmpty"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        Aucun joueur pour l'instant. Ajoutez des joueurs pour démarrer.
      </div>

      <div
        v-else-if="!hasResults"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        Aucun joueur ne correspond à « {{ searchQuery }} ».
      </div>

      <Table v-else>
        <TableHeader class="sticky top-0 z-10 bg-card/95 backdrop-blur">
          <TableRow>
            <TableHead class="w-16">#</TableHead>
            <TableHead>Joueur</TableHead>
            <TableHead>Sectorielles</TableHead>
            <TableHead class="text-right">ELO</TableHead>
            <TableHead class="text-right">Win rate</TableHead>
            <TableHead class="text-right">Nb parties</TableHead>
            <TableHead>Bilan</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow
            v-for="player in filteredPlayers"
            :key="player.name"
            class="player-row cursor-pointer"
            @click="emit('select', player)"
          >
            <TableCell>
              <Badge variant="outline" :class="rankBadgeClass(player.rank)">
                {{ player.rank }}
              </Badge>
            </TableCell>
            <TableCell class="font-medium">
              {{ player.display_name }}
              <span
                v-if="player.star_count"
                class="ml-1 text-[0.7em] leading-none text-amber-400"
                :title="`${player.star_count} victoire(s) en tournoi`"
              >
                {{ '⭐'.repeat(Math.min(player.star_count, 5)) }}
              </span>
            </TableCell>
            <TableCell>
              <div v-if="player.top_armies.length > 0" class="flex items-center gap-1.5">
                <ArmyLogo
                  v-for="usage in player.top_armies"
                  :key="usage.army_id"
                  :army-id="usage.army_id"
                  :title="armyTooltip(usage)"
                />
              </div>
              <span v-else class="text-muted-foreground">—</span>
            </TableCell>
            <TableCell class="text-right font-semibold tabular-nums elo-score">
              {{ Math.round(player.rating) }}
            </TableCell>
            <TableCell class="text-right font-semibold tabular-nums elo-score">
              {{ formatWinRate(winRate(player)) }}
            </TableCell>
            <TableCell class="text-right tabular-nums text-muted-foreground">
              {{ totalMatches(player) }}
            </TableCell>
            <TableCell class="min-w-[14rem]" @click.stop>
              <WinDrawLossBar
                compact
                omit-games-count
                :wins="player.wins"
                :draws="player.draws"
                :losses="player.losses"
              />
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </CardContent>
  </Card>
</template>
