<script setup lang="ts">
import { Check } from '@lucide/vue'
import type { PartieStep } from '@/composables/usePartieFlow'
import { PARTIE_STEP_LABELS } from '@/composables/usePartieFlow'
import { cn } from '@/lib/utils'

const props = defineProps<{
  steps: PartieStep[]
  currentIndex: number
}>()
</script>

<template>
  <ol class="partie-stepper" aria-label="Étapes de la partie">
    <li
      v-for="(stepId, index) in steps"
      :key="stepId"
      class="partie-stepper-item"
      :class="{
        'partie-stepper-item--active': index === currentIndex,
        'partie-stepper-item--done': index < currentIndex,
      }"
    >
      <span
        class="partie-stepper-badge"
        :class="
          cn(
            index < currentIndex && 'partie-stepper-badge--done',
            index === currentIndex && 'partie-stepper-badge--active',
          )
        "
        :aria-current="index === currentIndex ? 'step' : undefined"
      >
        <Check v-if="index < currentIndex" class="size-3.5" />
        <span v-else>{{ index + 1 }}</span>
      </span>
      <span class="partie-stepper-label">{{ PARTIE_STEP_LABELS[stepId] }}</span>
    </li>
  </ol>
</template>
