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
    | 'tournament_pool_id'
    | 'scenario_name'
    | 'counts_for_elo'
    | 'sync_pending'
  >
  /** Coupe le nom du tournoi à ce nombre de caractères, avec « ... ». */
  tournamentNameLimit?: number
  /** N’affiche que la phase (poule, ronde, finale), sans le nom du tournoi. */
  phaseOnly?: boolean
  /** Coupe le nom du scénario à ce nombre de caractères, avec « ... ». */
  scenarioNameLimit?: number
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

const tournamentFullLabel = computed(() => {
  if (props.phaseOnly) return phaseText.value
  const title = tournamentTitle.value
  if (!title) return null
  if (!phaseText.value) return title
  return `${title} - ${phaseText.value}`
})

const tournamentLabel = computed(() => {
  const full = tournamentFullLabel.value
  const limit = props.tournamentNameLimit
  if (!full || limit == null || [...full].length <= limit) return full
  return `${[...full].slice(0, limit).join('')}...`
})

const tournamentHover = computed(() =>
  tournamentLabel.value !== tournamentFullLabel.value
    ? tournamentFullLabel.value ?? undefined
    : undefined,
)

const scenarioName = computed(() => props.match.scenario_name?.trim() || null)

const scenarioLabel = computed(() => {
  const name = scenarioName.value
  const limit = props.scenarioNameLimit
  if (!name || limit == null || [...name].length <= limit) return name
  return `${[...name].slice(0, limit).join('')}...`
})

const scenarioHover = computed(() =>
  scenarioLabel.value !== scenarioName.value ? scenarioName.value ?? undefined : undefined,
)

const casualLabel = computed(() =>
  casualMatchContextLabel(props.match.counts_for_elo),
)

const phaseTo = computed(() => {
  const id = props.match.tournament_id
  if (id == null) return null
  if (
    props.phaseOnly
    && props.match.tournament_phase === 'pool'
    && props.match.tournament_pool_id != null
  ) {
    return {
      name: 'tournoi' as const,
      params: { id },
      query: { tab: 'poules', poolId: String(props.match.tournament_pool_id) },
    }
  }
  return { name: 'tournoi' as const, params: { id } }
})
</script>

<template>
  <div class="min-w-0 text-sm leading-snug">
    <template v-if="isTournamentMatch">
      <p class="truncate">
        <RouterLink
          v-if="match.tournament_id"
          :to="phaseTo ?? { name: 'tournoi', params: { id: match.tournament_id } }"
          class="font-medium text-primary hover:underline"
          :title="tournamentHover"
          @click.stop
        >
          {{ tournamentLabel }}
        </RouterLink>
        <span v-else class="font-medium" :title="tournamentHover">{{ tournamentLabel }}</span>
        <span v-if="phaseText && !phaseOnly && tournamentNameLimit == null" class="text-muted-foreground">
          {{ ` - ${phaseText}` }}
        </span>
      </p>
      <p v-if="scenarioLabel || match.sync_pending" class="truncate text-muted-foreground">
        <span v-if="scenarioLabel" :title="scenarioHover">{{ scenarioLabel }}</span>
        <template v-if="scenarioLabel && match.sync_pending"> · </template>
        <span v-if="match.sync_pending">En attente de synchro</span>
      </p>
    </template>
    <template v-else>
      <p class="truncate text-muted-foreground">{{ casualLabel }}</p>
      <p v-if="scenarioLabel || match.sync_pending" class="truncate text-muted-foreground">
        <span v-if="scenarioLabel" :title="scenarioHover">{{ scenarioLabel }}</span>
        <template v-if="scenarioLabel && match.sync_pending"> · </template>
        <span v-if="match.sync_pending">En attente de synchro</span>
      </p>
    </template>
  </div>
</template>
