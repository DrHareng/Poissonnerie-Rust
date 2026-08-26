<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { ChevronLeft, ChevronRight, FileText } from '@lucide/vue'
import type { RecentMatchReport } from '@/types/elo'
import ArmyLogo from '@/components/ArmyLogo.vue'
import MatchContextCell from '@/components/MatchContextCell.vue'
import { useArmies } from '@/composables/useArmies'
import { formatMatchRecordedDate } from '@/lib/tournamentMatchDisplay'
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
  reports: RecentMatchReport[]
  loading?: boolean
  page?: number
  pageSize?: number
  total?: number
  totalPages?: number
}>()

const emit = defineEmits<{
  pageChange: [page: number]
}>()

const router = useRouter()
const { ensureLoaded, getArmy } = useArmies()

const pageStart = computed(() => {
  if (!props.total || props.total === 0) return 0
  return ((props.page ?? 1) - 1) * (props.pageSize ?? 10) + 1
})

const pageEnd = computed(() => {
  if (!props.total || props.total === 0) return 0
  return Math.min((props.page ?? 1) * (props.pageSize ?? 10), props.total)
})

function armyName(armyId?: number | null): string {
  if (!armyId) return 'Sectorielle'
  return getArmy(armyId)?.name ?? 'Sectorielle'
}

function goToPage(nextPage: number) {
  if (!props.totalPages) return
  if (nextPage < 1 || nextPage > props.totalPages) return
  emit('pageChange', nextPage)
}

function openReport(report: RecentMatchReport) {
  router.push({
    name: 'match',
    params: { id: String(report.match_id) },
    hash: `#cr-${report.author_slot}`,
  })
}

onMounted(() => {
  void ensureLoaded()
})
</script>

<template>
  <Card class="neon-panel page-panel-scroll">
    <CardHeader class="lg:shrink-0">
      <CardTitle class="flex items-center gap-2">
        <FileText class="size-5 text-primary" />
        Comptes rendus
      </CardTitle>
      <CardDescription>
        <template v-if="total">
          {{ total }} compte{{ total > 1 ? 's' : '' }} rendu{{ total > 1 ? 's' : '' }} publié{{
            total > 1 ? 's' : ''
          }}, du plus récent au plus ancien.
        </template>
        <template v-else>
          Du plus récent au plus ancien.
        </template>
      </CardDescription>
    </CardHeader>
    <CardContent class="lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
      <div
        v-if="loading"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        Chargement des comptes rendus…
      </div>

      <div
        v-else-if="reports.length === 0"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        Aucun compte rendu publié pour l’instant.
      </div>

      <template v-else>
        <Table>
          <TableHeader class="sticky top-0 z-10 bg-card/95 backdrop-blur">
            <TableRow>
              <TableHead>Date</TableHead>
              <TableHead>Auteur</TableHead>
              <TableHead class="w-12">Secto</TableHead>
              <TableHead class="hidden sm:table-cell">Adversaire</TableHead>
              <TableHead class="hidden w-12 sm:table-cell">Secto</TableHead>
              <TableHead class="hidden md:table-cell">Contexte</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow
              v-for="report in reports"
              :key="report.report_id"
              class="player-row cursor-pointer"
              @click="openReport(report)"
            >
              <TableCell class="whitespace-nowrap text-muted-foreground">
                {{ formatMatchRecordedDate(report.updated_at || report.published_at) ?? '—' }}
              </TableCell>
              <TableCell class="min-w-0 truncate font-medium">
                {{ report.author_display_name || report.author_name }}
              </TableCell>
              <TableCell>
                <ArmyLogo
                  :army-id="report.author_army_id"
                  :title="armyName(report.author_army_id)"
                />
              </TableCell>
              <TableCell class="hidden min-w-0 truncate sm:table-cell">
                {{ report.opponent_display_name || report.opponent_name }}
              </TableCell>
              <TableCell class="hidden sm:table-cell">
                <ArmyLogo
                  :army-id="report.opponent_army_id"
                  :title="armyName(report.opponent_army_id)"
                />
              </TableCell>
              <TableCell class="hidden md:table-cell" @click.stop>
                <MatchContextCell :match="report" />
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>

        <div
          v-if="totalPages && totalPages > 1"
          class="mt-4 flex flex-col gap-3 border-t border-border pt-4 sm:flex-row sm:items-center sm:justify-between"
        >
          <p class="text-sm text-muted-foreground">
            {{ pageStart }}–{{ pageEnd }} sur {{ total }}
          </p>
          <div class="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              :disabled="(page ?? 1) <= 1 || loading"
              @click="goToPage((page ?? 1) - 1)"
            >
              <ChevronLeft class="size-4" />
              Précédent
            </Button>
            <span class="min-w-24 text-center text-sm text-muted-foreground">
              Page {{ page ?? 1 }} / {{ totalPages }}
            </span>
            <Button
              variant="outline"
              size="sm"
              :disabled="(page ?? 1) >= totalPages || loading"
              @click="goToPage((page ?? 1) + 1)"
            >
              Suivant
              <ChevronRight class="size-4" />
            </Button>
          </div>
        </div>
      </template>
    </CardContent>
  </Card>
</template>
