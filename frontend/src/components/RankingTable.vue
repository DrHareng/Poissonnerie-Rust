<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { Podium } from '@lucide/vue'
import type { PlayerArmyUsage, RankedPlayer } from '@/types/elo'
import ArmyLogo from '@/components/ArmyLogo.vue'
import WinDrawLossBar from '@/components/WinDrawLossBar.vue'
import { useArmies } from '@/composables/useArmies'
import { Badge } from '@/components/ui/badge'
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

onMounted(() => {
  void ensureLoaded()
})

const rankedActivePlayers = computed(() =>
  props.players
    .filter((player) => player.wins + player.draws + player.losses > 0)
    .map((player, index) => ({ ...player, rank: index + 1 })),
)

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
</script>

<template>
  <Card class="neon-panel page-panel-scroll">
    <CardHeader class="lg:shrink-0">
      <CardTitle class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div class="flex items-center gap-2">
          <Podium class="size-5 text-primary" />
          Classement ELO
        </div>
        <div
          v-if="$slots['header-actions']"
          class="flex w-full flex-col gap-2 sm:w-auto sm:flex-row sm:items-center"
        >
          <slot name="header-actions" />
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
            <TableCell class="min-w-[14rem]" @click.stop>
              <WinDrawLossBar
                compact
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
