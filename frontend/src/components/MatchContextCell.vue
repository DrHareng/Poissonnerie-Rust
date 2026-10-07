<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import type { MatchRecord } from '@/types/elo'
import { casualMatchContextLabel } from '@/lib/matchElo'
import { tournamentPhaseDisplay } from '@/lib/tournamentPhase'

const props = defineProps<{
  match: Pick<
    MatchRecord,
    | 'tournament_id'
    | 'tournament_name'
    | 'tournament_phase'
    | 'tournament_pool_name'
    | 'scenario_name'
    | 'counts_for_elo'
    | 'sync_pending'
  >
}>()

const isTournamentMatch = computed(
  () => props.match.tournament_id != null && props.match.tournament_phase,
)

const phaseText = computed(() => tournamentPhaseDisplay(props.match))

const tournamentTitle = computed(
  () =>
    props.match.tournament_name?.trim()
    || (props.match.tournament_id != null
      ? `Tournoi #${props.match.tournament_id}`
      : null),
)

const scenarioName = computed(() => props.match.scenario_name?.trim() || null)

const casualLabel = computed(() =>
  casualMatchContextLabel(props.match.counts_for_elo),
)

const secondaryLine = computed(() => {
  const parts: string[] = []
  if (scenarioName.value) parts.push(scenarioName.value)
  if (props.match.sync_pending) parts.push('En attente de synchro')
  return parts.length > 0 ? parts.join(' · ') : null
})
</script>

<template>
  <div class="min-w-0 text-sm leading-snug">
    <template v-if="isTournamentMatch">
      <p class="truncate">
        <RouterLink
          v-if="match.tournament_id"
          :to="{ name: 'tournoi', params: { id: match.tournament_id } }"
          class="font-medium text-primary hover:underline"
          @click.stop
        >
          {{ tournamentTitle }}
        </RouterLink>
        <span v-else class="font-medium">{{ tournamentTitle }}</span>
        <span v-if="phaseText" class="text-muted-foreground">
          {{ ` - ${phaseText}` }}
        </span>
      </p>
      <p v-if="secondaryLine" class="truncate text-muted-foreground">
        {{ secondaryLine }}
      </p>
    </template>
    <template v-else>
      <p class="truncate text-muted-foreground">{{ casualLabel }}</p>
      <p v-if="secondaryLine" class="truncate text-muted-foreground">
        {{ secondaryLine }}
      </p>
    </template>
  </div>
</template>
