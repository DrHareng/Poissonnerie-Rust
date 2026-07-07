<script setup lang="ts">
defineProps<{
  name: string
  displayName?: string | null
}>()
</script>

<template>
  <RouterLink
    :to="{ name: 'joueur', params: { name } }"
    class="player-link font-medium text-primary hover:underline"
    @click.stop
  >
    {{ displayName || name }}
  </RouterLink>
</template>
