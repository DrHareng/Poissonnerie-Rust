<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { Shield } from '@lucide/vue'
import type { PlayerArmyStats } from '@/types/elo'
import ArmyLogo from '@/components/ArmyLogo.vue'
import WinDrawLossBar from '@/components/WinDrawLossBar.vue'
import { useArmies } from '@/composables/useArmies'
import {
  fetchPrefs,
  updatePrefs,
  type ArmySortMode,
} from '@/lib/api'
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
  stats: PlayerArmyStats[]
  loading?: boolean
}>()

const router = useRouter()
const { ensureLoaded, getArmy } = useArmies()

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

const isEmpty = computed(() => props.stats.length === 0)

function totalMatches(entry: PlayerArmyStats) {
  return entry.wins + entry.draws + entry.losses
}

function otherSortMode(mode: ArmySortMode): ArmySortMode {
  return mode === 'win_rate' ? 'matches' : 'win_rate'
}

const sortedStats = computed(() => {
  const entries = [...props.stats]

  if (sortMode.value === 'win_rate') {
    entries.sort(
      (a, b) =>
        b.win_rate - a.win_rate ||
        totalMatches(b) - totalMatches(a) ||
        a.army_id - b.army_id,
    )
  } else {
    entries.sort(
      (a, b) =>
        totalMatches(b) - totalMatches(a) ||
        b.win_rate - a.win_rate ||
        a.army_id - b.army_id,
    )
  }

  return entries
})

function formatWinRate(winRate: number) {
  return `${winRate.toLocaleString('fr-FR', {
    minimumFractionDigits: 0,
    maximumFractionDigits: 1,
  })} %`
}

function formatEloDelta(delta: number) {
  const rounded = Math.round(delta)
  if (rounded > 0) return `+${rounded}`
  return String(rounded)
}

function eloDeltaClass(delta: number) {
  const rounded = Math.round(delta)
  if (rounded < 0) return 'match-elo-delta--loss'
  if (rounded > 0) return 'match-elo-delta--gain'
  return 'text-muted-foreground'
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

function openSectorielle(armyId: number) {
  router.push({ name: 'sectorielle', params: { id: armyId } })
}
</script>

<template>
  <Card size="sm" class="neon-panel shrink-0">
    <CardHeader>
      <CardTitle class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div class="flex items-center gap-2">
          <Shield class="size-5 text-primary" />
          Statistiques par sectorielle
        </div>
        <div class="flex items-center gap-2">
          <span class="text-sm font-medium text-muted-foreground">Tri :</span>
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
      </CardTitle>
    </CardHeader>
    <CardContent>
      <div
        v-if="loading"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        Chargement des statistiques...
      </div>

      <div
        v-else-if="isEmpty"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        Aucune sectorielle enregistrée pour ce joueur.
      </div>

      <Table v-else>
        <TableHeader class="sticky top-0 z-10 bg-card/95 backdrop-blur">
          <TableRow>
            <TableHead>Sectorielle</TableHead>
            <TableHead class="text-right">Win rate</TableHead>
            <TableHead class="text-right">Nb parties</TableHead>
            <TableHead>Bilan</TableHead>
            <TableHead class="text-right">Variation ELO</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow
            v-for="entry in sortedStats"
            :key="entry.army_id"
            class="player-row cursor-pointer"
            @click="openSectorielle(entry.army_id)"
          >
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
            <TableCell class="min-w-[14rem]">
              <WinDrawLossBar
                compact
                omit-games-count
                :wins="entry.wins"
                :draws="entry.draws"
                :losses="entry.losses"
              />
            </TableCell>
            <TableCell
              class="text-right tabular-nums"
              :class="eloDeltaClass(entry.elo_delta)"
            >
              {{ formatEloDelta(entry.elo_delta) }}
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </CardContent>
  </Card>
</template>
