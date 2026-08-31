<script setup lang="ts">
import { computed } from 'vue'
import type { TournamentTopFourEntry } from '@/types/elo'
import { topFourDisplayRows } from '@/lib/tournamentDisplay'
import ArmyLogo from '@/components/ArmyLogo.vue'
import PlayerLink from '@/components/PlayerLink.vue'

const props = defineProps<{
  entries?: TournamentTopFourEntry[] | null
}>()

const displayRows = computed(() => topFourDisplayRows(props.entries ?? []))

function rowLabel(label: string) {
  if (label === '1') return '1er'
  if (label === '2') return '2e'
  return label
}
</script>

<template>
  <div v-if="displayRows.length > 0" class="space-y-1.5 text-xs">
    <template v-for="row in displayRows" :key="row.label">
      <template v-if="row.label === '3-4'">
        <div class="flex min-w-0 gap-2">
          <span
            class="flex w-7 shrink-0 items-center self-stretch tabular-nums text-muted-foreground"
          >
            3-4
          </span>
          <div class="flex min-w-0 flex-1 flex-col gap-1">
            <div
              v-for="entry in row.entries"
              :key="entry.player_name"
              class="flex min-w-0 items-center gap-2"
            >
              <ArmyLogo :army-id="entry.army_id ?? undefined" class="!size-5 shrink-0" />
              <PlayerLink
                :name="entry.player_name"
                :display-name="entry.player_display_name"
                class="min-w-0 truncate"
              />
            </div>
          </div>
        </div>
      </template>
      <div
        v-else
        class="flex min-w-0 items-center gap-2"
      >
        <span class="w-7 shrink-0 tabular-nums text-muted-foreground">
          {{ rowLabel(row.label) }}
        </span>
        <ArmyLogo :army-id="row.entries[0]!.army_id ?? undefined" class="!size-5" />
        <PlayerLink
          :name="row.entries[0]!.player_name"
          :display-name="row.entries[0]!.player_display_name"
          class="min-w-0 truncate"
        />
      </div>
    </template>
  </div>
  <p v-else class="text-xs text-muted-foreground">
    Podium non disponible.
  </p>
</template>
