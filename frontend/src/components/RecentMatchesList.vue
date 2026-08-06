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
  matches: MatchRecord[]
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

const pageStart = computed(() => {
  if (!props.total || props.total === 0) return 0
  return ((props.page ?? 1) - 1) * (props.pageSize ?? 5) + 1
})

const pageEnd = computed(() => {
  if (!props.total || props.total === 0) return 0
  return Math.min((props.page ?? 1) * (props.pageSize ?? 5), props.total)
})

function goToPage(nextPage: number) {
  if (!props.totalPages) return
  if (nextPage < 1 || nextPage > props.totalPages) return
  emit('pageChange', nextPage)
}

function openMatch(id: number) {
  router.push({ name: 'match', params: { id: String(id) } })
}
</script>

<template>
  <Card class="neon-panel page-panel-scroll">
    <CardHeader class="lg:shrink-0">
      <CardTitle class="flex items-center gap-2">
        <History class="size-5 text-primary" />
        Matchs enregistrés
      </CardTitle>
      <CardDescription>
        <template v-if="total">
          {{ total }} match{{ total > 1 ? 's' : '' }} au total, du plus récent au plus ancien.
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
        Chargement des matchs...
      </div>

      <div
        v-else-if="matches.length === 0"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        Aucun match enregistré pour l'instant.
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
              <TableHead class="w-12 text-right">
                <span class="sr-only">Actions</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="match in matches" :key="match.id">
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
                <MatchResultBadges :match="match" />
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
