<script setup lang="ts">
import type { SecondaryObjective } from '@/types/elo'
import { secondaryImageSrc } from '@/lib/secondaryImages'

defineProps<{
  secondaries: SecondaryObjective[]
  selectable?: boolean
  selectedSlug?: string
}>()

const emit = defineEmits<{
  select: [slug: string]
}>()
</script>

<template>
  <div class="secondary-card-grid">
    <button
      v-for="secondary in secondaries"
      :key="secondary.id"
      type="button"
      class="neon-panel secondary-card shrink-0 overflow-hidden p-0 text-left"
      :class="{
        'secondary-card--clickable': selectable && !!secondaryImageSrc(secondary.slug),
        'ring-2 ring-primary': selectable && selectedSlug === secondary.slug,
        'opacity-50': selectable && !secondaryImageSrc(secondary.slug),
      }"
      :disabled="!selectable || !secondaryImageSrc(secondary.slug)"
      @click="selectable ? emit('select', secondary.slug) : undefined"
    >
      <img
        v-if="secondaryImageSrc(secondary.slug)"
        :src="secondaryImageSrc(secondary.slug)"
        :alt="secondary.name"
        class="secondary-card-image"
      />
      <div v-else class="p-3 text-sm font-medium">{{ secondary.name }}</div>
    </button>
  </div>
</template>
