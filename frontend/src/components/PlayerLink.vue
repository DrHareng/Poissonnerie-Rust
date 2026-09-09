<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    name: string
    displayName?: string | null
    linked?: boolean
    adversaire?: string | null
  }>(),
  { linked: true },
)

const isLinked = computed(() => {
  if (!props.linked) return false
  const guest = props.adversaire?.trim().toLowerCase()
  if (guest && props.name.trim().toLowerCase() === guest) return false
  return true
})
</script>

<template>
  <span v-if="!isLinked" class="font-medium">{{ displayName || name }}</span>
  <RouterLink
    v-else
    :to="{ name: 'joueur', params: { name } }"
    class="player-link font-medium text-primary hover:underline"
    @click.stop
  >
    {{ displayName || name }}
  </RouterLink>
</template>
