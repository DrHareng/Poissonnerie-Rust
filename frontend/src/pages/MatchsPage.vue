<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { Eye, Play, Trash2 } from '@lucide/vue'
import { deleteMatch, fetchRecentMatches, fetchRecentReports } from '@/lib/api'
import type { MatchRecord, RecentMatchReport } from '@/types/elo'
import RecentMatchesList from '@/components/RecentMatchesList.vue'
import RecentReportsList from '@/components/RecentReportsList.vue'
import ArmyLogo from '@/components/ArmyLogo.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import MatchContextCell from '@/components/MatchContextCell.vue'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import { useAuth } from '@/composables/useAuth'
import { useMyInProgressMatches, inProgressMenuLabel } from '@/composables/useMyInProgressMatches'
import { PARTIE_STEP_LABELS, type PartieStep } from '@/composables/usePartieFlow'
import { matchsTabs } from '@/lib/pageTitleTabs'
import { formatMatchRecordedDate } from '@/lib/tournamentMatchDisplay'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
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

const PAGE_SIZE = 5
const REPORT_PAGE_SIZE = 10

const router = useRouter()
const route = useRoute()
const { isAuthenticated, isAdmin } = useAuth()
const {
  allMatches,
  myMatches,
  loading: loadingInProgress,
  refresh: refreshInProgress,
} = useMyInProgressMatches()

const inProgress = computed(() =>
  isAdmin.value ? allMatches.value : myMatches.value,
)
const deletingId = ref<number | null>(null)
const apiOnline = ref(true)

const matches = ref<MatchRecord[]>([])
const totalMatches = ref(0)
const page = ref(1)
const loadingMatches = ref(true)

const isReportsTab = computed(() => route.name === 'matchs-cr')

const reports = ref<RecentMatchReport[]>([])
const totalReports = ref(0)
const reportsPage = ref(1)
const loadingReports = ref(true)

const totalPages = computed(() => Math.max(1, Math.ceil(totalMatches.value / PAGE_SIZE)))
const totalReportPages = computed(() =>
  Math.max(1, Math.ceil(totalReports.value / REPORT_PAGE_SIZE)),
)

const inProgressTitle = computed(() => {
  const count = inProgress.value.length
  if (isAdmin.value) {
    return count <= 1 ? `${count} partie en cours` : `${count} parties en cours`
  }
  return inProgressMenuLabel(count)
})

const inProgressDescription = computed(() =>
  isAdmin.value
    ? 'Toutes les parties non terminées. Vous pouvez les supprimer en cas de bug.'
    : 'Reprenez une partie commencée via l’assistant.',
)

function stepLabel(step: string | null | undefined): string {
  if (!step) return 'En cours'
  return PARTIE_STEP_LABELS[step as PartieStep] ?? step
}

async function refreshMatches() {
  loadingMatches.value = true
  try {
    const response = await fetchRecentMatches(PAGE_SIZE, (page.value - 1) * PAGE_SIZE)
    matches.value = response.items
    totalMatches.value = response.total
    apiOnline.value = true
  } catch (error) {
    apiOnline.value = false
    toast.error(error instanceof Error ? error.message : 'Impossible de charger les matchs')
  } finally {
    loadingMatches.value = false
  }
}

async function refreshReports() {
  loadingReports.value = true
  try {
    const response = await fetchRecentReports(
      REPORT_PAGE_SIZE,
      (reportsPage.value - 1) * REPORT_PAGE_SIZE,
    )
    reports.value = response.items
    totalReports.value = response.total
    apiOnline.value = true
  } catch (error) {
    apiOnline.value = false
    toast.error(
      error instanceof Error ? error.message : 'Impossible de charger les comptes rendus',
    )
  } finally {
    loadingReports.value = false
  }
}

async function refreshAll() {
  if (isReportsTab.value) {
    await Promise.all([refreshReports(), refreshInProgress()])
    return
  }
  await Promise.all([refreshMatches(), refreshInProgress()])
}

function onPageChange(nextPage: number) {
  page.value = nextPage
}

function onReportsPageChange(nextPage: number) {
  reportsPage.value = nextPage
}

function resumePartie(id: number) {
  router.push({ name: 'partie-resume', params: { id: String(id) } })
}

function openMatch(id: number) {
  router.push({ name: 'match', params: { id: String(id) } })
}

async function onDeleteInProgress(id: number) {
  if (!isAdmin.value) return
  if (!window.confirm('Supprimer cette partie en cours ?')) return

  deletingId.value = id
  try {
    await deleteMatch(id)
    toast.success('Partie supprimée')
    await Promise.all([refreshInProgress(), refreshMatches()])
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Suppression impossible')
  } finally {
    deletingId.value = null
  }
}

async function scrollToHash() {
  const hash = route.hash
  if (!hash) return
  await nextTick()
  document.querySelector(hash)?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

watch(page, () => {
  if (!isReportsTab.value) void refreshMatches()
})
watch(reportsPage, () => {
  if (isReportsTab.value) void refreshReports()
})
watch(isReportsTab, (isReports) => {
  if (isReports) {
    if (reportsPage.value !== 1) {
      reportsPage.value = 1
      return
    }
    void refreshReports()
    return
  }
  if (page.value !== 1) {
    page.value = 1
    return
  }
  void refreshMatches()
})
watch(() => route.hash, scrollToHash)
watch(loadingInProgress, (loading) => {
  if (!loading && route.hash === '#parties-en-cours') {
    void scrollToHash()
  }
})

onMounted(async () => {
  await refreshAll()
  await scrollToHash()
})
</script>

<template>
  <div class="page-stack">
    <PageTitleTabs
      :tabs="matchsTabs"
      ariaLabel="Sections des matchs"
    />

    <Alert v-if="!apiOnline" variant="destructive" class="neon-panel-accent">
      <AlertTitle>API indisponible</AlertTitle>
      <AlertDescription>
        Lancez le serveur Rust avec
        <code class="rounded bg-muted px-1 py-0.5">cargo run --bin poissonnerie-server</code>
        puis rechargez la page.
      </AlertDescription>
    </Alert>

    <Card
      v-if="isAuthenticated && !isReportsTab"
      id="parties-en-cours"
      class="neon-panel scroll-mt-24"
    >
      <CardHeader>
        <CardTitle class="flex items-center gap-2">
          <Play class="size-5 text-primary" />
          {{ inProgressTitle }}
        </CardTitle>
        <CardDescription>
          {{ inProgressDescription }}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div v-if="loadingInProgress" class="text-sm text-muted-foreground">
          Chargement…
        </div>
        <p v-else-if="inProgress.length === 0" class="text-sm text-muted-foreground">
          Aucune partie en cours.
        </p>
        <Table v-else>
          <TableHeader>
            <TableRow>
              <TableHead>Match</TableHead>
              <TableHead>Adversaires</TableHead>
              <TableHead class="hidden md:table-cell">Contexte</TableHead>
              <TableHead class="hidden sm:table-cell">Étape</TableHead>
              <TableHead class="w-36 text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="item in inProgress" :key="item.id">
              <TableCell class="tabular-nums">
                #{{ item.id }}
                <div class="text-xs text-muted-foreground">
                  {{ formatMatchRecordedDate(item.recorded_at) ?? '—' }}
                </div>
              </TableCell>
              <TableCell>
                <div class="flex flex-col gap-1">
                  <div class="flex items-center gap-2">
                    <ArmyLogo :army-id="item.player1_army_id" />
                    <PlayerLink
                      :name="item.player1"
                      :display-name="item.player1_display_name"
                    />
                  </div>
                  <div class="flex items-center gap-2">
                    <ArmyLogo :army-id="item.player2_army_id" />
                    <PlayerLink
                      :name="item.player2"
                      :display-name="item.player2_display_name"
                    />
                  </div>
                </div>
              </TableCell>
              <TableCell class="hidden md:table-cell">
                <MatchContextCell :match="item" />
              </TableCell>
              <TableCell class="hidden sm:table-cell">
                <Badge variant="secondary">{{ stepLabel(item.partie_step) }}</Badge>
              </TableCell>
              <TableCell class="text-right">
                <div class="inline-flex gap-1">
                  <Button
                    type="button"
                    size="icon"
                    variant="outline"
                    title="Reprendre"
                    @click="resumePartie(item.id)"
                  >
                    <Play class="size-4" />
                  </Button>
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    title="Voir le match"
                    @click="openMatch(item.id)"
                  >
                    <Eye class="size-4" />
                  </Button>
                  <Button
                    v-if="isAdmin"
                    type="button"
                    size="icon"
                    variant="destructive"
                    title="Supprimer"
                    :disabled="deletingId === item.id"
                    @click="onDeleteInProgress(item.id)"
                  >
                    <Trash2 class="size-4" />
                  </Button>
                </div>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </CardContent>
    </Card>

    <RecentReportsList
      v-if="isReportsTab"
      :reports="reports"
      :loading="loadingReports"
      :page="reportsPage"
      :page-size="REPORT_PAGE_SIZE"
      :total="totalReports"
      :total-pages="totalReportPages"
      @page-change="onReportsPageChange"
    />
    <RecentMatchesList
      v-else
      :matches="matches"
      :loading="loadingMatches"
      :page="page"
      :page-size="PAGE_SIZE"
      :total="totalMatches"
      :total-pages="totalPages"
      @page-change="onPageChange"
    />
  </div>
</template>
