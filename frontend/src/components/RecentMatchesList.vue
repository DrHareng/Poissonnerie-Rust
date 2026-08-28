<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { ChevronLeft, ChevronRight, Eye, History } from '@lucide/vue'
import type { MatchRecord } from '@/types/elo'
import MatchResultBadges from '@/components/MatchResultBadges.vue'
import MatchContextCell from '@/components/MatchContextCell.vue'
import ArmyLogo from '@/components/ArmyLogo.vue'
import ArmyListQuickActions from '@/components/ArmyListQuickActions.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import { matchCountsForElo } from '@/lib/matchElo'
import {
  normalizeMatchForPlayer,
  playerMatchEloDelta,
} from '@/lib/matchPlayerPerspective'
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

const props = withDefaults(
  defineProps<{
    matches: MatchRecord[]
    loading?: boolean
    page?: number
    pageSize?: number
    total?: number
    totalPages?: number
    /** Normalise les matchs pour afficher ce joueur en J1. */
    perspectivePlayer?: string
    /** Colonne évolution ELO (J1). */
    showElo?: boolean
    title?: string
    description?: string
    emptyMessage?: string
    /** Pagination locale sur la liste reçue. */
    clientSide?: boolean
    /** Sans carte englobante (intégration dans une page existante). */
    bare?: boolean
  }>(),
  {
    pageSize: 5,
    title: 'Matchs enregistrés',
    emptyMessage: "Aucun match enregistré pour l'instant.",
  },
)

const emit = defineEmits<{
  pageChange: [page: number]
}>()

const router = useRouter()

const preparedMatches = computed(() => {
  let list = props.matches
  if (props.perspectivePlayer) {
    list = list
      .filter((match) => match.status !== 'in_progress')
      .map((match) => normalizeMatchForPlayer(match, props.perspectivePlayer!))
  }
  return list
})

const effectiveTotal = computed(() =>
  props.clientSide ? preparedMatches.value.length : (props.total ?? props.matches.length),
)

const effectiveTotalPages = computed(() => {
  if (props.clientSide) {
    return Math.max(1, Math.ceil(preparedMatches.value.length / props.pageSize))
  }
  return props.totalPages ?? 1
})

const visibleMatches = computed(() => {
  if (!props.clientSide) return props.matches
  const start = ((props.page ?? 1) - 1) * props.pageSize
  return preparedMatches.value.slice(start, start + props.pageSize)
})

const displayMatches = computed(() =>
  props.clientSide ? visibleMatches.value : preparedMatches.value,
)

const pageStart = computed(() => {
  if (!effectiveTotal.value) return 0
  return ((props.page ?? 1) - 1) * props.pageSize + 1
})

const pageEnd = computed(() => {
  if (!effectiveTotal.value) return 0
  return Math.min((props.page ?? 1) * props.pageSize, effectiveTotal.value)
})

const defaultDescription = computed(() => {
  if (effectiveTotal.value) {
    return `${effectiveTotal.value} match${effectiveTotal.value > 1 ? 's' : ''} au total, du plus récent au plus ancien.`
  }
  return 'Du plus récent au plus ancien.'
})

function goToPage(nextPage: number) {
  if (nextPage < 1 || nextPage > effectiveTotalPages.value) return
  emit('pageChange', nextPage)
}

function openMatch(id: number) {
  router.push({ name: 'match', params: { id: String(id) } })
}

function eloDeltaClass(match: MatchRecord) {
  const delta = playerMatchEloDelta(match)
  if (delta != null && delta < 0) return 'match-elo-delta--loss'
  if (delta != null && delta > 0) return 'match-elo-delta--gain'
  return undefined
}

function formatEloCell(match: MatchRecord) {
  if (!matchCountsForElo(match.counts_for_elo)) return null
  const delta = playerMatchEloDelta(match)
  if (delta == null) return null
  const oldRating = Math.round(match.player1_old)
  const newRating = Math.round(match.player1_new)
  const deltaLabel = delta > 0 ? `+${Math.round(delta)}` : String(Math.round(delta))
  return { oldRating, newRating, deltaLabel }
}
</script>

<template>
  <Card
    class="neon-panel page-panel-scroll"
    :class="{ 'border-0 bg-transparent shadow-none': bare }"
  >
    <CardHeader v-if="!bare" class="lg:shrink-0">
      <CardTitle class="flex items-center gap-2">
        <History class="size-5 text-primary" />
        {{ title }}
      </CardTitle>
      <CardDescription>
        {{ description ?? defaultDescription }}
      </CardDescription>
    </CardHeader>
    <CardContent
      class="lg:min-h-0 lg:flex-1 lg:overflow-y-auto"
      :class="{ 'px-0': bare }"
    >
      <div
        v-if="loading"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        Chargement des matchs...
      </div>

      <div
        v-else-if="displayMatches.length === 0"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        {{ emptyMessage }}
      </div>

      <template v-else>
        <Table>
          <TableHeader class="sticky top-0 z-10 bg-card/95 backdrop-blur">
            <TableRow>
              <TableHead>Date</TableHead>
              <TableHead class="text-right">Joueur 1</TableHead>
              <TableHead class="w-10" aria-hidden="true" />
              <TableHead class="text-center">Résultat</TableHead>
              <TableHead class="w-10" aria-hidden="true" />
              <TableHead>Joueur 2</TableHead>
              <TableHead>Contexte</TableHead>
              <TableHead v-if="showElo">ELO</TableHead>
              <TableHead class="w-12 text-right">
                <span class="sr-only">Actions</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="match in displayMatches" :key="match.id">
              <TableCell class="whitespace-nowrap text-muted-foreground">
                {{ formatMatchRecordedDate(match.recorded_at) ?? '—' }}
              </TableCell>
              <TableCell class="text-right">
                <PlayerLink
                  :name="match.player1"
                  :display-name="match.player1_display_name"
                />
              </TableCell>
              <TableCell class="px-2">
                <div class="flex items-center gap-1">
                  <ArmyLogo :army-id="match.player1_army_id" />
                  <ArmyListQuickActions
                    :code="match.player1_army_list_code"
                    icon-only
                  />
                </div>
              </TableCell>
              <TableCell>
                <MatchResultBadges
                  :match="match"
                  :emphasize-defeat="!!perspectivePlayer"
                />
              </TableCell>
              <TableCell class="px-2">
                <div class="flex items-center gap-1">
                  <ArmyLogo :army-id="match.player2_army_id" />
                  <ArmyListQuickActions
                    :code="match.player2_army_list_code"
                    icon-only
                  />
                </div>
              </TableCell>
              <TableCell>
                <PlayerLink
                  :name="match.player2"
                  :display-name="match.player2_display_name"
                />
              </TableCell>
              <TableCell>
                <MatchContextCell :match="match" />
              </TableCell>
              <TableCell
                v-if="showElo"
                class="tabular-nums"
              >
                <template v-if="formatEloCell(match)">
                  {{ formatEloCell(match)!.oldRating }}
                  →
                  {{ formatEloCell(match)!.newRating }}
                  <span
                    class="text-xs"
                    :class="eloDeltaClass(match)"
                  >
                    ({{ formatEloCell(match)!.deltaLabel }})
                  </span>
                </template>
                <span v-else class="text-muted-foreground">—</span>
              </TableCell>
              <TableCell class="text-right">
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  :title="`Voir le match #${match.id}`"
                  :aria-label="`Voir le match #${match.id}`"
                  @click="openMatch(match.id)"
                >
                  <Eye class="size-4" />
                </Button>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>

        <div
          v-if="effectiveTotalPages > 1"
          class="mt-4 flex flex-col gap-3 border-t border-border pt-4 sm:flex-row sm:items-center sm:justify-between"
        >
          <p class="text-sm text-muted-foreground">
            {{ pageStart }}–{{ pageEnd }} sur {{ effectiveTotal }}
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
              Page {{ page ?? 1 }} / {{ effectiveTotalPages }}
            </span>
            <Button
              variant="outline"
              size="sm"
              :disabled="(page ?? 1) >= effectiveTotalPages || loading"
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
