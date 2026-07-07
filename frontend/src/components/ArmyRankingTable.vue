<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { Shield } from '@lucide/vue'
import type { RankedArmy } from '@/types/elo'
import ArmyLogo from '@/components/ArmyLogo.vue'
import WinDrawLossBar from '@/components/WinDrawLossBar.vue'
import { useArmies } from '@/composables/useArmies'
import { Badge } from '@/components/ui/badge'
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

onMounted(() => {
  void ensureLoaded()
})

const isEmpty = computed(() => props.armies.length === 0)

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
</script>

<template>
  <Card class="neon-panel page-panel-scroll">
    <CardHeader class="lg:shrink-0">
      <CardTitle class="flex items-center gap-2">
        <Shield class="size-5 text-primary" />
        Classement des sectorielles
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
            <TableHead>Bilan</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow
            v-for="entry in armies"
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
