<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import type { MatchRecord } from '@/types/elo'
import { casualMatchContextLabel } from '@/lib/matchElo'
import { phaseLabel } from '@/lib/tournamentPhase'

const props = defineProps<{
  match: Pick<
    MatchRecord,
    | 'tournament_id'
    | 'tournament_name'
    | 'tournament_phase'
    | 'scenario_name'
    | 'counts_for_elo'
  >
}>()

const isTournamentMatch = computed(
  () => props.match.tournament_id != null && props.match.tournament_phase,
)

const phaseText = computed(() => phaseLabel(props.match.tournament_phase))

const contextLine = computed(() => {
  const parts: string[] = []

  if (isTournamentMatch.value && phaseText.value) {
    parts.push(phaseText.value)
  } else if (!isTournamentMatch.value) {
    parts.push(casualMatchContextLabel(props.match.counts_for_elo))
  }

  if (props.match.scenario_name) {
    parts.push(props.match.scenario_name)
  }

  return parts.join(' · ')
})
</script>

<template>
  <div class="min-w-0 text-sm">
    <template v-if="isTournamentMatch">
      <RouterLink
        v-if="match.tournament_id"
        :to="{ name: 'tournoi', params: { id: match.tournament_id } }"
        class="block truncate font-medium text-primary hover:underline"
      >
        {{ match.tournament_name ?? `Tournoi #${match.tournament_id}` }}
      </RouterLink>
      <p class="truncate text-muted-foreground">
        {{ contextLine }}
      </p>
    </template>
    <p v-else class="truncate text-muted-foreground">
      {{ contextLine }}
    </p>
  </div>
</template>
