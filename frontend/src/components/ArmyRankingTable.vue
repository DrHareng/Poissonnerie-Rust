<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Shield } from '@lucide/vue'
import type { RankedArmy } from '@/types/elo'
import ArmyLogo from '@/components/ArmyLogo.vue'
import WinDrawLossBar from '@/components/WinDrawLossBar.vue'
import { useArmies } from '@/composables/useArmies'
import {
  fetchPrefs,
  updatePrefs,
  type ArmySortMode,
} from '@/lib/api'
import { Badge } from '@/components/ui/badge'
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

const props = defineProps<{
  armies: RankedArmy[]
  loading?: boolean
}>()

const emit = defineEmits<{
  select: [army: RankedArmy]
}>()

const { ensureLoaded, getArmy } = useArmies()

const isEmpty = computed(() => props.armies.length === 0)

const sortMode = ref<ArmySortMode>('win_rate')

onMounted(() => {
  void ensureLoaded()
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

function rankBadgeClass(rank: number) {
  if (rank === 1) return 'rank-badge-gold tabular-nums font-semibold'
  if (rank === 2) return 'rank-badge-silver tabular-nums font-semibold'
  if (rank === 3) return 'rank-badge-bronze tabular-nums font-semibold'
  return 'rank-badge-outline tabular-nums'
}

function formatWinRate(winRate: number) {
  return `${winRate.toLocaleString('fr-FR', {
    minimumFractionDigits: 0,
    maximumFractionDigits: 1,
  })} %`
}

function totalMatches(entry: RankedArmy) {
  return entry.wins + entry.draws + entry.losses
}

function otherSortMode(mode: ArmySortMode): ArmySortMode {
  return mode === 'win_rate' ? 'matches' : 'win_rate'
}

const displayEntries = computed(() => {
  const entries = [...props.armies]

  if (sortMode.value === 'win_rate') {
    entries.sort((a, b) => b.win_rate - a.win_rate || totalMatches(b) - totalMatches(a))
  } else {
    entries.sort(
      (a, b) =>
        totalMatches(b) - totalMatches(a) ||
        b.win_rate - a.win_rate ||
        a.army_id - b.army_id,
    )
  }

  return entries.map((entry, index) => ({
    ...entry,
    rank: index + 1,
  }))
})

function sortButtonVariant(mode: ArmySortMode) {
  return sortMode.value === mode ? 'outline' : 'outline'
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
</script>

<template>
  <Card class="neon-panel page-panel-scroll">
    <CardHeader class="lg:shrink-0">
      <CardTitle class="flex items-center justify-between gap-4">
        <div class="flex items-center gap-2">
          <Shield class="size-5 text-primary" />
          Classement des sectorielles
        </div>
        <div class="flex items-center gap-2">
          <span class="text-sm font-medium text-muted-foreground">Tri :</span>
          <div class="flex items-center gap-0">
            <Button
              type="button"
              size="xs"
              :variant="sortButtonVariant('win_rate')"
              :class="[
                'rounded-r-none',
                sortButtonClass('win_rate'),
              ]"
              @click="setSortMode('win_rate')"
            >
              Win rate
            </Button>
            <Button
              type="button"
              size="xs"
              :variant="sortButtonVariant('matches')"
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
      </CardTitle>
      <CardDescription>
        Win rate calculé sur l'ensemble des parties enregistrées avec une sectorielle.
      </CardDescription>
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
        Aucune sectorielle n'a encore été jouée.
      </div>

      <Table v-else>
        <TableHeader class="sticky top-0 z-10 bg-card/95 backdrop-blur">
          <TableRow>
            <TableHead class="w-16">#</TableHead>
            <TableHead>Sectorielle</TableHead>
            <TableHead class="text-right">Win rate</TableHead>
            <TableHead class="text-right">Nb parties</TableHead>
            <TableHead>Bilan</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow
            v-for="entry in displayEntries"
            :key="entry.army_id"
            class="player-row cursor-pointer"
            @click="emit('select', entry)"
          >
            <TableCell>
              <Badge variant="outline" :class="rankBadgeClass(entry.rank)">
                {{ entry.rank }}
              </Badge>
            </TableCell>
            <TableCell>
              <div class="flex items-center gap-2 font-medium">
                <ArmyLogo :army-id="entry.army_id" />
                {{ getArmy(entry.army_id)?.name ?? `Sectorielle #${entry.army_id}` }}
              </div>
            </TableCell>
            <TableCell class="text-right font-semibold tabular-nums elo-score">
              {{ formatWinRate(entry.win_rate) }}
            </TableCell>
            <TableCell class="text-right tabular-nums text-muted-foreground">
              {{ totalMatches(entry) }}
            </TableCell>
            <TableCell class="min-w-[14rem]" @click.stop>
              <WinDrawLossBar
                compact
                :wins="entry.wins"
                :draws="entry.draws"
                :losses="entry.losses"
              />
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </CardContent>
  </Card>
</template>
