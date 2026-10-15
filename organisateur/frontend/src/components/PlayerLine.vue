<script setup lang="ts">
import { computed } from 'vue'
import { armyLogoUrl } from '@/format'
import type { PlayerCard, Sectorial } from '@/types'

const props = defineProps<{
  player: PlayerCard
  sectorials: Sectorial[]
}>()

const logo = computed(() => {
  const slug = props.sectorials.find((item) => item.id === props.player.sectorial_id)?.slug
  return slug ? armyLogoUrl(slug) : ''
})
</script>

<template>
  <div class="player-line">
    <img
      v-if="logo"
      class="army-logo"
      :src="logo"
      alt=""
      :title="player.sectorial_name"
    />
    <strong class="clip">{{ player.name }}</strong>
    <slot />
  </div>
</template>
