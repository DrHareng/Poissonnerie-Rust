<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  wins: number
  draws: number
  losses: number
  compact?: boolean
}>()

const total = computed(() => props.wins + props.draws + props.losses)

function label(count: number, singular: string, plural: string) {
  return count > 1 ? plural : singular
}

const detailLabel = computed(() => {
  const parties = `${total.value} ${label(total.value, 'partie', 'parties')}`
  const victoires = `${props.wins} ${label(props.wins, 'victoire', 'victoires')}`
  const nuls = `${props.draws} ${label(props.draws, 'nul', 'nuls')}`
  const defaites = `${props.losses} ${label(props.losses, 'défaite', 'défaites')}`
  return `${parties} - ${victoires}, ${nuls}, ${defaites}`
})

const segments = computed(() => {
  if (total.value === 0) {
    return []
  }

  return [
  { key: 'wins', count: props.wins, class: 'wnd-bar-win' },
  { key: 'draws', count: props.draws, class: 'wnd-bar-draw' },
  { key: 'losses', count: props.losses, class: 'wnd-bar-loss' },
  ]
    .filter((segment) => segment.count > 0)
    .map((segment) => ({
      ...segment,
      width: `${(segment.count / total.value) * 100}%`,
    }))
})

</script>

<template>
  <div class="wnd-bar" :class="{ 'wnd-bar--compact': compact }">
    <p v-if="!compact" class="wnd-bar-summary">
      {{ detailLabel }}
    </p>
    <div class="wnd-bar-track" :title="detailLabel">
      <template v-if="total > 0">
        <div
          v-for="segment in segments"
          :key="segment.key"
          class="wnd-bar-segment"
          :class="segment.class"
          :style="{ width: segment.width }"
        />
      </template>
      <div v-else class="wnd-bar-empty" />
    </div>
    <p v-if="compact" class="wnd-bar-ratio">
      {{ detailLabel }}
    </p>
  </div>
</template>
