<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import { List } from '@lucide/vue'
import type { Army, ArmyListStatsEntry } from '@/types/elo'
import ArmyLogo from '@/components/ArmyLogo.vue'
import ArmyListMatchesPanel from '@/components/ArmyListMatchesPanel.vue'
import ArmyListQuickActions from '@/components/ArmyListQuickActions.vue'
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

void ensureLoaded()

const armyName = computed(() => {
  if (!props.armyId) return null
  return getArmy(props.armyId)?.name ?? `Sectorielle #${props.armyId}`
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
          :armies="armies"
          :disabled="armiesLoading || armies.length === 0"
          :placeholder="
            armiesLoading
              ? 'Chargement…'
              : 'Tapez pour chercher une armée'
          "
        />
      </div>
    </CardHeader>
    <CardContent>
      <div
        v-if="loading"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        Chargement des listes…
      </div>

      <p
        v-else-if="!armyId"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        Choisissez une sectorielle.
      </p>

      <p
        v-else-if="lists.length === 0"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        Aucune liste enregistrée pour cette sectorielle.
      </p>

      <Table v-else>
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
          <template v-for="entry in lists" :key="entry.id">
            <TableRow>
              <TableCell class="max-w-[14rem] truncate text-sm" :title="entry.code">
                {{ listLabel(entry) }}
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
    </CardContent>
  </Card>
</template>
