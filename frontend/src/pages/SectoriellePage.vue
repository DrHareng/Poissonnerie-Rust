<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useTitle } from '@vueuse/core'
import { ArrowLeft } from '@lucide/vue'
import { toast } from 'vue-sonner'
import { fetchArmyMatches, fetchArmyStats } from '@/lib/api'
import { pageTitle } from '@/lib/pageTitle'
import { formatMatchRecordedDate } from '@/lib/tournamentMatchDisplay'
import type { MatchOutcome, MatchRecord, RankedArmy } from '@/types/elo'
import MatchResultBadges from '@/components/MatchResultBadges.vue'
import MatchContextCell from '@/components/MatchContextCell.vue'
import WinDrawLossBar from '@/components/WinDrawLossBar.vue'
import ArmyLogo from '@/components/ArmyLogo.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import { useArmies } from '@/composables/useArmies'
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
const { ensureLoaded, getArmy } = useArmies()

const army = ref<RankedArmy | null>(null)
const matches = ref<MatchRecord[]>([])
const loading = ref(true)
const loadingMatches = ref(true)

const armyId = computed(() => Number(route.params.id))

function flipOutcome(outcome: MatchOutcome): MatchOutcome {
  if (outcome === 'player1_win') return 'player2_win'
  if (outcome === 'player2_win') return 'player1_win'
  return 'draw'
}

function normalizeMatchForArmy(match: MatchRecord, sectorialId: number): MatchRecord {
  if (match.player1_army_id === sectorialId) {
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
  if (!army.value) return []

  return matches.value.map((match) => ({
    id: match.id,
    date: formatMatchRecordedDate(match.recorded_at) ?? '—',
    normalized: normalizeMatchForArmy(match, army.value!.army_id),
  }))
})

const armyName = computed(
  () => getArmy(armyId.value)?.name ?? `Sectorielle #${armyId.value}`,
)

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

async function loadArmy() {
  const id = armyId.value
  if (!Number.isFinite(id) || id <= 0) {
    army.value = null
    return
  }

  loading.value = true
  try {
    await ensureLoaded()
    army.value = await fetchArmyStats(id)
  } catch (error) {
    army.value = null
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
    return
  }

  loadingMatches.value = true
  try {
    matches.value = await fetchArmyMatches(id)
  } catch {
    matches.value = []
  } finally {
    loadingMatches.value = false
  }
}

async function refresh() {
  await Promise.all([loadArmy(), loadMatches()])
}

watch(armyId, refresh, { immediate: true })
onMounted(refresh)
</script>

<template>
  <div class="page-stack">
    <Button variant="ghost" class="w-fit" @click="router.back()">
      <ArrowLeft class="size-4" />
      Retour
    </Button>

    <div
      v-if="loading"
      class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
    >
      Chargement de la sectorielle...
    </div>

    <template v-else-if="army">
      <section class="page-header">
        <div class="flex items-center gap-3">
          <ArmyLogo :army-id="army.army_id" class="!size-10" />
          <div>
            <h1 class="page-title">{{ armyName }}</h1>
            <p class="page-description">
              Rang #{{ army.rank }} — {{ formatWinRate(army.win_rate) }} win rate
            </p>
          </div>
        </div>
      </section>

      <WinDrawLossBar
        :wins="army.wins"
        :draws="army.draws"
        :losses="army.losses"
      />

      <Card class="neon-panel page-panel-scroll">
        <CardHeader class="lg:shrink-0">
          <CardTitle>Historique des parties</CardTitle>
          <CardDescription>
            Toutes les parties enregistrées avec cette sectorielle.
          </CardDescription>
        </CardHeader>
        <CardContent class="lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
          <div
            v-if="loadingMatches"
            class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
          >
            Chargement des parties...
          </div>

          <div
            v-else-if="matches.length === 0"
            class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
          >
            Aucune partie enregistrée pour cette sectorielle.
          </div>

          <Table v-else>
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
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="row in matchRows" :key="row.id">
                <TableCell class="whitespace-nowrap text-muted-foreground">
                  {{ row.date }}
                </TableCell>
                <TableCell class="text-right">
                  <PlayerLink
                    :name="row.normalized.player1"
                    :display-name="row.normalized.player1_display_name"
                  />
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
                  {{ Math.round(row.normalized.player1_old) }}
                  →
                  <span class="elo-score">{{ Math.round(row.normalized.player1_new) }}</span>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </template>
  </div>
</template>
