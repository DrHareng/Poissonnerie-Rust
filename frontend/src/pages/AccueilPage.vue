<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { FileText, History, Medal, Trophy } from '@lucide/vue'
import { toast } from 'vue-sonner'
import {
  fetchRanking,
  fetchRecentMatches,
  fetchRecentReports,
  fetchTournaments,
} from '@/lib/api'
import type {
  MatchRecord,
  PlayerArmyUsage,
  RankedPlayer,
  RecentMatchReport,
  TournamentListEntry,
} from '@/types/elo'
import ArmyLogo from '@/components/ArmyLogo.vue'
import ArmyListQuickActions from '@/components/ArmyListQuickActions.vue'
import BracketTree from '@/components/BracketTree.vue'
import MatchResultBadges from '@/components/MatchResultBadges.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import TournamentPoolScenarioLinks from '@/components/TournamentPoolScenarioLinks.vue'
import { useArmies } from '@/composables/useArmies'
import { casualMatchContextLabel } from '@/lib/matchElo'
import { formatRegistrationSummary, tournamentRegistrationCapacity } from '@/lib/tournamentDisplay'
import { phaseLabel } from '@/lib/tournamentPhase'
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
  TableRow,
} from '@/components/ui/table'

const router = useRouter()
const { ensureLoaded, getArmy } = useArmies()

const loading = ref(true)
const players = ref<RankedPlayer[]>([])
const tournament = ref<TournamentListEntry | null>(null)
const matches = ref<MatchRecord[]>([])
const reports = ref<RecentMatchReport[]>([])

const topPlayers = computed(() =>
  players.value
    .filter((player) => player.wins + player.draws + player.losses > 0)
    .slice(0, 10)
    .map((player, index) => ({ ...player, rank: index + 1 })),
)

function scoreLabel(objectives: number, survivors: number) {
  return `${objectives} - ${survivors}`
}

/** Largeur commune des badges score (max sur les 5 parties). */
const scoreBadgeMinCh = computed(() => {
  let max = 0
  for (const match of matches.value) {
    if (match.status === 'in_progress' || !match.outcome) {
      max = Math.max(max, Math.ceil('En cours'.length / 2))
      continue
    }
    max = Math.max(
      max,
      scoreLabel(match.player1_objectives, match.player1_survivors).length,
      scoreLabel(match.player2_objectives, match.player2_survivors).length,
    )
  }
  return Math.max(max, 1)
})

function rankBadgeClass(rank: number) {
  if (rank === 1) return 'rank-badge-gold tabular-nums font-semibold'
  if (rank === 2) return 'rank-badge-silver tabular-nums font-semibold'
  if (rank === 3) return 'rank-badge-bronze tabular-nums font-semibold'
  return 'rank-badge-outline tabular-nums'
}

function armyName(armyId?: number | null): string {
  if (!armyId) return 'Sectorielle'
  return getArmy(armyId)?.name ?? 'Sectorielle'
}

function armyTooltip(usage: PlayerArmyUsage) {
  const armyName = getArmy(usage.army_id)?.name ?? 'cette sectorielle'
  const label = usage.matches > 1 ? 'parties' : 'partie'
  return `${usage.matches} ${label} avec ${armyName}`
}

function openPlayer(player: RankedPlayer) {
  router.push({ name: 'joueur', params: { name: player.name } })
}

function openMatch(id: number) {
  if (matches.value.find((match) => match.id === id)?.status === 'in_progress') {
    router.push({ name: 'partie-resume', params: { id: String(id) } })
    return
  }
  router.push({ name: 'match', params: { id: String(id) } })
}

function openReport(report: RecentMatchReport) {
  router.push({
    name: 'match',
    params: { id: String(report.match_id) },
    hash: `#cr-${report.author_slot}`,
  })
}

function openTournament() {
  if (!tournament.value) return
  router.push({ name: 'tournoi', params: { id: tournament.value.id } })
}

function matchContextLabel(match: MatchRecord) {
  const parts: string[] = []
  if (match.tournament_id != null && match.tournament_phase) {
    const phase = phaseLabel(match.tournament_phase)
    if (phase) parts.push(phase)
  } else {
    parts.push(casualMatchContextLabel(match.counts_for_elo))
  }
  if (match.scenario_name) parts.push(match.scenario_name)
  return parts.join(' · ')
}

function matchShortDate(timestamp: number) {
  if (!timestamp || timestamp < 31_536_000) return null
  const date = new Date(timestamp * 1000)
  if (date.getFullYear() === 1970) return null
  return new Intl.DateTimeFormat('fr-FR', {
    day: '2-digit',
    month: '2-digit',
  }).format(date)
}

onMounted(async () => {
  loading.value = true
  try {
    const [ranking, tournaments, recent, recentReports] = await Promise.all([
      fetchRanking(),
      fetchTournaments(),
      fetchRecentMatches(5),
      fetchRecentReports(5),
      ensureLoaded(),
    ])
    players.value = ranking
    tournament.value =
      tournaments.find((item) => item.status !== 'draft') ?? tournaments[0] ?? null
    matches.value = recent.items
    reports.value = recentReports.items ?? []
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible de charger l’accueil',
    )
  } finally {
    loading.value = false
  }
})
</script>

<template>
  <div class="page-stack">
    <div
      class="grid min-h-0 flex-1 grid-cols-1 items-start gap-4 lg:grid-cols-[minmax(0,20rem)_minmax(0,1fr)] xl:grid-cols-[minmax(0,22rem)_minmax(0,1fr)]"
    >
      <div class="grid w-full min-w-0 gap-4 self-start">
      <Card class="neon-panel h-fit w-full self-start">
        <CardHeader class="pb-3">
          <div class="flex items-center justify-between gap-3">
            <CardTitle class="flex items-center gap-2">
              <Trophy class="size-5 text-primary" />
              Top 10
            </CardTitle>
            <RouterLink
              to="/classement"
              class="text-sm font-medium text-primary hover:underline"
            >
              Voir tout
            </RouterLink>
          </div>
        </CardHeader>
        <CardContent>
          <div
            v-if="loading"
            class="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground"
          >
            Chargement…
          </div>
          <div
            v-else-if="topPlayers.length === 0"
            class="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground"
          >
            Aucun joueur classé pour l’instant.
          </div>
          <Table v-else>
            <TableBody>
              <TableRow
                v-for="player in topPlayers"
                :key="player.name"
                class="player-row cursor-pointer"
                @click="openPlayer(player)"
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
                  <div
                    v-if="player.top_armies.length > 0"
                    class="flex items-center gap-1.5"
                  >
                    <ArmyLogo
                      v-for="usage in player.top_armies"
                      :key="usage.army_id"
                      :army-id="usage.army_id"
                      :title="armyTooltip(usage)"
                    />
                  </div>
                  <span v-else class="text-muted-foreground">—</span>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <Card class="neon-panel h-fit w-full">
        <CardHeader class="pb-3">
          <div class="flex items-center justify-between gap-3">
            <CardTitle class="flex items-center gap-2">
              <FileText class="size-5 text-primary" />
              Derniers comptes rendus
            </CardTitle>
            <RouterLink
              :to="{ name: 'matchs-cr' }"
              class="text-sm font-medium text-primary hover:underline"
            >
              Voir tout
            </RouterLink>
          </div>
        </CardHeader>
        <CardContent>
          <div
            v-if="loading"
            class="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground"
          >
            Chargement…
          </div>
          <div
            v-else-if="reports.length === 0"
            class="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground"
          >
            Aucun compte rendu publié pour l’instant.
          </div>
          <Table v-else>
            <TableBody>
              <TableRow
                v-for="report in reports"
                :key="report.report_id"
                class="player-row cursor-pointer"
                @click="openReport(report)"
              >
                <TableCell class="tabular-nums text-muted-foreground">
                  {{ matchShortDate(report.updated_at || report.published_at) ?? '—' }}
                </TableCell>
                <TableCell class="min-w-0 truncate font-medium">
                  {{ report.author_display_name || report.author_name }}
                </TableCell>
                <TableCell class="text-right">
                  <ArmyLogo
                    :army-id="report.author_army_id"
                    :title="armyName(report.author_army_id)"
                  />
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </CardContent>
      </Card>
      </div>

      <div class="grid min-w-0 gap-4 self-start">
        <Card class="neon-panel">
          <CardHeader class="pb-3">
            <div class="flex items-center justify-between gap-3">
              <CardTitle class="flex items-center gap-2">
                <Medal class="size-5 text-primary" />
                Dernier tournoi
              </CardTitle>
              <RouterLink
                to="/tournois"
                class="text-sm font-medium text-primary hover:underline"
              >
                Voir tout
              </RouterLink>
            </div>
          </CardHeader>
          <CardContent>
            <div
              v-if="loading"
              class="rounded-lg border border-dashed p-6 text-center text-sm text-muted-foreground"
            >
              Chargement…
            </div>
            <div
              v-else-if="!tournament"
              class="rounded-lg border border-dashed p-6 text-center text-sm text-muted-foreground"
            >
              Aucun tournoi.
            </div>
            <button
              v-else
              type="button"
              class="grid w-full gap-3 rounded-lg border p-4 text-left transition hover:border-primary/50 hover:bg-muted/30"
              @click="openTournament"
            >
              <div class="flex items-start justify-between gap-4">
                <div class="min-w-0 space-y-1">
                  <p class="font-medium">{{ tournament.name }}</p>
                  <p class="text-sm text-muted-foreground">
                    {{
                      formatRegistrationSummary(
                        tournament.registered_count,
                        tournament.waitlist_count,
                        tournamentRegistrationCapacity(tournament.pool_count),
                      )
                    }}
                  </p>
                </div>
                <Badge variant="outline" class="shrink-0">
                  {{ tournament.display_status }}
                </Badge>
              </div>
              <div
                v-if="tournament.description?.trim()"
                class="prose prose-sm max-w-none text-left text-muted-foreground"
              >
                <MarkdownContent :source="tournament.description" />
              </div>
              <div
                v-if="(tournament.pool_scenarios?.length ?? 0) > 0"
                class="space-y-1 text-left"
              >
                <p class="text-xs font-medium text-muted-foreground">Scénarios de poules</p>
                <TournamentPoolScenarioLinks :scenarios="tournament.pool_scenarios ?? []" />
              </div>
              <BracketTree
                v-if="tournament.bracket_matches?.length"
                :matches="tournament.bracket_matches"
                compact
              />
            </button>
          </CardContent>
        </Card>

        <Card class="neon-panel">
          <CardHeader class="pb-2">
            <div class="flex items-center justify-between gap-3">
              <CardTitle class="flex items-center gap-2">
                <History class="size-5 text-primary" />
                Dernières parties
              </CardTitle>
              <RouterLink
                to="/matchs"
                class="text-sm font-medium text-primary hover:underline"
              >
                Voir tout
              </RouterLink>
            </div>
          </CardHeader>
          <CardContent class="pt-0">
            <div
              v-if="loading"
              class="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground"
            >
              Chargement…
            </div>
            <div
              v-else-if="matches.length === 0"
              class="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground"
            >
              Aucune partie pour l’instant.
            </div>
            <div v-else class="divide-y divide-border/60 rounded-lg border">
              <div
                v-for="match in matches"
                :key="match.id"
                role="link"
                tabindex="0"
                class="grid w-full cursor-pointer grid-cols-[2.35rem_minmax(0,1fr)_auto_minmax(0,1fr)] items-center gap-x-1 px-2 py-1 text-left transition hover:bg-muted/30 sm:grid-cols-[2.35rem_minmax(0,1fr)_auto_minmax(0,1fr)_minmax(4.5rem,0.65fr)] sm:gap-x-1.5 sm:px-2 sm:py-1.5"
                @click="openMatch(match.id)"
                @keydown.enter.prevent="openMatch(match.id)"
                @keydown.space.prevent="openMatch(match.id)"
              >
                <span class="tabular-nums text-[11px] text-muted-foreground">
                  {{ matchShortDate(match.recorded_at) ?? '—' }}
                </span>
                <div class="flex min-w-0 items-center justify-end gap-1">
                  <PlayerLink
                    :name="match.player1"
                    :display-name="match.player1_display_name"
                    class="truncate text-xs"
                  />
                  <ArmyLogo :army-id="match.player1_army_id" />
                  <ArmyListQuickActions
                    :code="match.player1_army_list_code"
                    icon-only
                    class="shrink-0"
                    @click.stop
                  />
                </div>
                <div class="home-match-score shrink-0 [&_.mx-auto]:mx-0 [&_[data-slot=badge]]:h-5 [&_[data-slot=badge]]:px-1.5 [&_[data-slot=badge]]:text-[10px] [&_[data-slot=badge]]:leading-none">
                  <MatchResultBadges
                    :match="match"
                    :badge-min-ch="scoreBadgeMinCh"
                  />
                </div>
                <div class="flex min-w-0 items-center gap-1">
                  <ArmyLogo :army-id="match.player2_army_id" />
                  <ArmyListQuickActions
                    :code="match.player2_army_list_code"
                    icon-only
                    class="shrink-0"
                    @click.stop
                  />
                  <PlayerLink
                    :name="match.player2"
                    :display-name="match.player2_display_name"
                    class="truncate text-xs"
                  />
                </div>
                <span
                  class="hidden truncate justify-self-end text-[11px] text-muted-foreground sm:block"
                >
                  {{ matchContextLabel(match) }}
                </span>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  </div>
</template>
