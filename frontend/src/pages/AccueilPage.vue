<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { FileText, History, Podium, Trophy } from '@lucide/vue'
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
import BracketTree from '@/components/BracketTree.vue'
import MatchListRow from '@/components/MatchListRow.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import TournamentPoolScenarioLinks from '@/components/TournamentPoolScenarioLinks.vue'
import { useArmies } from '@/composables/useArmies'
import { scoreBadgeMinCh as computeScoreBadgeMinCh } from '@/lib/matchResultBadges'
import { mainNavLinks } from '@/lib/mainNav'
import { formatPoolsEndSummary, formatRegistrationSummary, isTournamentPoolsPhase, isTournamentRegistrationPhase, tournamentRegistrationCapacity } from '@/lib/tournamentDisplay'
import TournamentDescriptionWithRegistrants from '@/components/TournamentDescriptionWithRegistrants.vue'
import TournamentPoolsPreview from '@/components/TournamentPoolsPreview.vue'
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
const navLinks = mainNavLinks

const topPlayers = computed(() =>
  players.value
    .filter((player) => player.wins + player.draws + player.losses > 0)
    .slice(0, 10)
    .map((player, index) => ({ ...player, rank: index + 1 })),
)

/** Largeur commune des badges score (max sur les 5 parties). */
const scoreBadgeMinCh = computed(() => computeScoreBadgeMinCh(matches.value))

const matchListStyle = computed(() => ({
  '--match-score-track': `calc(${scoreBadgeMinCh.value * 2}ch + 2.5rem)`,
}))

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

function openTournamentPool(poolId: number) {
  if (!tournament.value) return
  void router.push({
    name: 'tournoi',
    params: { id: tournament.value.id },
    query: { tab: 'poules', poolId: String(poolId) },
  })
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
    <nav class="home-nav-grid" aria-label="Menu principal">
      <RouterLink
        v-for="link in navLinks"
        :key="link.to"
        :to="link.to"
        class="home-nav-tile"
      >
        <component :is="link.icon" class="size-6 text-primary" />
        <span class="home-nav-tile-label">{{ link.label }}</span>
      </RouterLink>
    </nav>

    <div
      class="hidden min-h-0 flex-1 grid-cols-1 items-start gap-4 md:grid lg:grid-cols-[minmax(0,1fr)_minmax(0,20rem)] xl:grid-cols-[minmax(0,1fr)_minmax(0,22rem)]"
    >
      <div class="grid min-w-0 gap-4 self-start">
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
            <div
              v-else
              class="home-match-list divide-y divide-border/60 rounded-lg border"
              :style="matchListStyle"
            >
              <MatchListRow
                v-for="match in matches"
                :key="match.id"
                :match="match"
                :badge-min-ch="scoreBadgeMinCh"
                compact
              />
            </div>
          </CardContent>
        </Card>

        <Card class="neon-panel">
          <CardHeader class="pb-3">
            <div class="flex items-center justify-between gap-3">
              <CardTitle class="flex items-center gap-2">
                <Trophy class="size-5 text-primary" />
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
                  <p
                    v-if="isTournamentPoolsPhase(tournament) && formatPoolsEndSummary(tournament)"
                    class="text-sm text-muted-foreground"
                  >
                    {{ formatPoolsEndSummary(tournament) }}
                  </p>
                  <p
                    v-else-if="!isTournamentPoolsPhase(tournament)"
                    class="text-sm text-muted-foreground"
                  >
                    {{
                      formatRegistrationSummary(
                        tournament.registered_count,
                        tournament.waitlist_count,
                        tournamentRegistrationCapacity(
                          tournament.pool_count,
                          tournament.structure,
                        ),
                      )
                    }}
                  </p>
                </div>
                <Badge variant="outline" class="shrink-0">
                  {{ tournament.display_status }}
                </Badge>
              </div>
              <TournamentPoolsPreview
                v-if="isTournamentPoolsPhase(tournament)"
                :pools="tournament.pools ?? []"
                :registrations="tournament.registrations ?? []"
                :matches="tournament.pool_matches ?? []"
                :qualified-per-pool="tournament.qualified_per_pool ?? 0"
                stacked
                selectable
                @click.stop
                @select-pool="openTournamentPool"
              />
              <TournamentDescriptionWithRegistrants
                v-else-if="
                  isTournamentRegistrationPhase(tournament.status)
                  && (tournament.description?.trim()
                    || (tournament.registrations?.length ?? 0) > 0
                    || (tournament.pool_scenarios?.length ?? 0) > 0)
                "
                :description="tournament.description"
                :registrations="tournament.registrations ?? []"
                :scenarios="tournament.pool_scenarios ?? []"
                compact
              />
              <div
                v-else-if="tournament.description?.trim()"
                class="prose prose-sm max-w-none text-left text-muted-foreground"
              >
                <MarkdownContent :source="tournament.description" />
              </div>
              <div
                v-if="
                  (tournament.pool_scenarios?.length ?? 0) > 0
                  && !isTournamentRegistrationPhase(tournament.status)
                  && !isTournamentPoolsPhase(tournament)
                "
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
      </div>

      <div class="grid w-full min-w-0 gap-4 self-start">
        <Card class="neon-panel h-fit w-full self-start">
          <CardHeader class="pb-3">
            <div class="flex items-center justify-between gap-3">
              <CardTitle class="flex items-center gap-2">
                <Podium class="size-5 text-primary" />
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
                Derniers rapports
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
              Aucun rapport publié pour l’instant.
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
    </div>
  </div>
</template>
