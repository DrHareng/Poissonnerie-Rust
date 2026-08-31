<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { Shield } from '@lucide/vue'
import type { PlayerArmyStats } from '@/types/elo'
import ArmyLogo from '@/components/ArmyLogo.vue'
import WinDrawLossBar from '@/components/WinDrawLossBar.vue'
import { useArmies } from '@/composables/useArmies'
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
  stats: PlayerArmyStats[]
  loading?: boolean
}>()

const router = useRouter()
const { ensureLoaded, getArmy } = useArmies()

onMounted(() => {
  void ensureLoaded()
})

const isEmpty = computed(() => props.stats.length === 0)

function totalMatches(entry: PlayerArmyStats) {
  return entry.wins + entry.draws + entry.losses
}

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

function openSectorielle(armyId: number) {
  router.push({ name: 'sectorielle', params: { id: armyId } })
}
</script>

<template>
  <Card size="sm" class="neon-panel shrink-0">
    <CardHeader>
      <CardTitle class="flex items-center gap-2">
        <Shield class="size-5 text-primary" />
        Statistiques par sectorielle
      </CardTitle>
      <CardDescription>
        Classées par win rate décroissant.
      </CardDescription>
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
            v-for="entry in stats"
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
