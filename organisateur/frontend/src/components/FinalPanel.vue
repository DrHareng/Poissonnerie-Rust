<script setup lang="ts">
import RoundResults from '@/components/RoundResults.vue'
import type { RoundCard } from '@/types'

defineProps<{
  rounds: RoundCard[]
}>()
</script>

<template>
  <section class="stack">
    <details v-for="round in rounds" :key="round.round" open>
      <summary>Ronde {{ round.round }}</summary>
      <div class="card">
        <RoundResults :matches="round.matches" />
      </div>
    </details>
  </section>
</template>
