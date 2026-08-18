<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import type { TournamentScenarioSlot } from '@/types/elo'

const props = defineProps<{
  scenarios: TournamentScenarioSlot[]
}>()

const items = computed(() =>
  props.scenarios.filter((slot) => slot.scenario_name?.trim()),
)

function scenarioTo(slot: TournamentScenarioSlot) {
  const slug = slot.scenario_slug?.trim()
  if (slug) return `/scenarios/${encodeURIComponent(slug)}`
  return '/scenarios'
}
</script>

<template>
  <p
    v-if="items.length > 0"
    class="text-sm text-muted-foreground"
  >
    <template
      v-for="(slot, index) in items"
      :key="`${slot.kind}-${slot.slot}-${slot.scenario_id}`"
    >
      <span
        v-if="index > 0"
        aria-hidden="true"
      > · </span>
      <RouterLink
        :to="scenarioTo(slot)"
        class="text-primary underline-offset-2 hover:underline"
        @click.stop
      >
        {{ slot.scenario_name }}
      </RouterLink>
    </template>
  </p>
</template>
