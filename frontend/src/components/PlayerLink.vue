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

const label = computed(() => {
  const shown = props.displayName?.trim()
  return shown || props.name
})

const isLinked = computed(() => {
  if (!props.linked) return false
  const guest = props.adversaire?.trim().toLowerCase()
  if (guest && props.name.trim().toLowerCase() === guest) return false
  return true
})
</script>

<template>
  <span
    v-if="!isLinked"
    class="player-link font-medium"
    :title="label"
  >{{ label }}</span>
  <RouterLink
    v-else
    :to="{ name: 'joueur', params: { name } }"
    class="player-link font-medium text-primary hover:underline"
    :title="label"
    @click.stop
  >
    {{ label }}
  </RouterLink>
</template>
