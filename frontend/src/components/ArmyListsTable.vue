<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { List } from '@lucide/vue'
import type { Army, ArmyListStatsEntry } from '@/types/elo'
import ArmyLogo from '@/components/ArmyLogo.vue'
import ArmyListMatchesPanel from '@/components/ArmyListMatchesPanel.vue'
import ArmyListQuickActions from '@/components/ArmyListQuickActions.vue'
import PaginationBar from '@/components/PaginationBar.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import SectorialPicker from '@/components/SectorialPicker.vue'
import WinDrawLossBar from '@/components/WinDrawLossBar.vue'
import { useArmies } from '@/composables/useArmies'
import { formatMatchRecordedDate } from '@/lib/tournamentMatchDisplay'
import { parseArmyListName } from '@/lib/armyList'
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

const PAGE_SIZE = 10

const props = defineProps<{
  armyId: number | null
  lists: ArmyListStatsEntry[]
  armies: Army[]
  loading?: boolean
  armiesLoading?: boolean
}>()

const emit = defineEmits<{
  'update:armyId': [value: number | null]
}>()

const router = useRouter()
const { ensureLoaded, getArmy } = useArmies()
const expandedListId = ref<number | null>(null)
const page = ref(1)

void ensureLoaded()

const armyName = computed(() => {
  if (!props.armyId) return null
  return getArmy(props.armyId)?.name ?? `Sectorielle #${props.armyId}`
})

const totalPages = computed(() =>
  Math.max(1, Math.ceil(props.lists.length / PAGE_SIZE)),
)

const pageLists = computed(() => {
  const start = (page.value - 1) * PAGE_SIZE
  return props.lists.slice(start, start + PAGE_SIZE)
})

const pickerValue = computed({
  get() {
    return props.armyId != null ? String(props.armyId) : undefined
  },
  set(value: string | undefined) {
    if (value == null || value === '') {
      emit('update:armyId', null)
      return
    }
    const id = Number(value)
    emit('update:armyId', Number.isFinite(id) ? id : null)
  },
})

watch(
  () => props.armyId,
  () => {
    page.value = 1
    expandedListId.value = null
  },
)

watch(
  () => props.lists,
  () => {
    if (page.value > totalPages.value) {
      page.value = totalPages.value
    }
    if (
      expandedListId.value != null &&
      !props.lists.some((entry) => entry.id === expandedListId.value)
    ) {
      expandedListId.value = null
    }
  },
)

function formatWinRate(winRate: number) {
  return `${winRate.toLocaleString('fr-FR', {
    minimumFractionDigits: 0,
    maximumFractionDigits: 1,
  })} %`
}

function openSectorielle() {
  if (!props.armyId) return
  router.push({ name: 'sectorielle', params: { id: props.armyId } })
}

function listLabel(entry: ArmyListStatsEntry) {
  const name = entry.name?.trim() || parseArmyListName(entry.code)?.trim()
  return name || entry.code
}

function toggleDetail(listId: number) {
  expandedListId.value = expandedListId.value === listId ? null : listId
}

function goToPage(nextPage: number) {
  const max = totalPages.value
  if (nextPage < 1 || nextPage > max) return
  page.value = nextPage
  expandedListId.value = null
}
</script>

<template>
  <Card class="neon-panel">
    <CardHeader>
      <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div class="min-w-0 space-y-1.5">
          <CardTitle class="flex items-center gap-2">
            <button
              v-if="armyId"
              type="button"
              class="inline-flex items-center gap-2 text-left hover:underline"
              @click="openSectorielle"
            >
              <ArmyLogo :army-id="armyId" />
              {{ armyName }}
            </button>
            <span v-else>Listes d'armée</span>
          </CardTitle>
          <CardDescription>
            Statistiques issues des matchs enregistrés (hors tournois en cours).
          </CardDescription>
        </div>
        <SectorialPicker
          v-model="pickerValue"
          class="w-full shrink-0 sm:w-64"
          allow-empty
          empty-label="Toutes"
          :armies="armies"
          :disabled="armiesLoading || armies.length === 0"
          :placeholder="
            armiesLoading
              ? 'Chargement…'
              : 'Toutes les sectorielles'
          "
        />
      </div>
    </CardHeader>
    <CardContent class="space-y-4">
      <div
        v-if="loading"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        Chargement des listes…
      </div>

      <p
        v-else-if="lists.length === 0"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        {{
          armyId
            ? 'Aucune liste enregistrée pour cette sectorielle.'
            : 'Aucune liste enregistrée.'
        }}
      </p>

      <template v-else>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Liste</TableHead>
              <TableHead>Joueur</TableHead>
              <TableHead class="text-right">Win rate</TableHead>
              <TableHead class="text-right">Parties</TableHead>
              <TableHead>Bilan</TableHead>
              <TableHead class="text-right">Dernière utilisation</TableHead>
              <TableHead class="w-36 text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <template v-for="entry in pageLists" :key="entry.id">
              <TableRow>
                <TableCell class="max-w-[16rem] text-sm" :title="entry.code">
                  <div class="flex min-w-0 items-center gap-2">
                    <ArmyLogo
                      v-if="!armyId"
                      :army-id="entry.army_id"
                    />
                    <span class="truncate">{{ listLabel(entry) }}</span>
                  </div>
                </TableCell>
                <TableCell class="max-w-[10rem] truncate text-sm">
                  <PlayerLink
                    v-if="entry.origin_player"
                    :name="entry.origin_player"
                    :display-name="entry.origin_player_display_name"
                  />
                  <span v-else class="text-muted-foreground">—</span>
                </TableCell>
                <TableCell class="text-right font-semibold tabular-nums elo-score">
                  {{ formatWinRate(entry.win_rate) }}
                </TableCell>
                <TableCell class="text-right tabular-nums text-muted-foreground">
                  {{ entry.games }}
                </TableCell>
                <TableCell class="min-w-[12rem]">
                  <WinDrawLossBar
                    compact
                    omit-games-count
                    :wins="entry.wins"
                    :draws="entry.draws"
                    :losses="entry.losses"
                  />
                </TableCell>
                <TableCell class="text-right text-xs text-muted-foreground tabular-nums">
                  {{ formatMatchRecordedDate(entry.last_used_at) ?? '—' }}
                </TableCell>
                <TableCell class="text-right">
                  <div class="inline-flex items-center justify-end gap-1">
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      :class="{
                        'border-primary/40 bg-primary/10': expandedListId === entry.id,
                      }"
                      @click="toggleDetail(entry.id)"
                    >
                      <List class="size-3.5" />
                      Détail
                    </Button>
                    <ArmyListQuickActions :code="entry.code" icon-only />
                  </div>
                </TableCell>
              </TableRow>
              <TableRow v-if="expandedListId === entry.id">
                <TableCell colspan="7" class="p-0">
                  <ArmyListMatchesPanel :list-id="entry.id" />
                </TableCell>
              </TableRow>
            </template>
          </TableBody>
        </Table>

        <PaginationBar
          v-if="totalPages > 1"
          :page="page"
          :total-pages="totalPages"
          :total="lists.length"
          :page-size="PAGE_SIZE"
          :loading="loading"
          @page-change="goToPage"
        />
      </template>
    </CardContent>
  </Card>
</template>
