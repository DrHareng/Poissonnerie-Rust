<script setup lang="ts">
import { ref, useId } from 'vue'
import WheelColumn from '@/components/WheelColumn.vue'
import { watchOverlay } from '@/overlay'

defineProps<{
  label: string
}>()

const hours = defineModel<number>('hours', { required: true })
const minutes = defineModel<number>('minutes', { required: true })

const labelId = useId()
const open = ref(false)
watchOverlay(open)

function pad(value: number) {
  return String(value).padStart(2, '0')
}
</script>

<template>
  <div class="wheel">
    <span :id="labelId">{{ label }}</span>
    <button type="button" class="wheel-field" :aria-labelledby="labelId" @click="open = true">
      {{ pad(hours) }}:{{ pad(minutes) }}
    </button>
    <Teleport to="body">
      <div v-if="open" class="wheel-backdrop" @click.self="open = false">
        <div class="wheel-sheet" role="dialog" :aria-label="label">
          <div class="wheel-pair">
            <WheelColumn v-model="hours" :min="0" :max="12" label="Heures" pad />
            <WheelColumn v-model="minutes" :min="0" :max="59" label="Minutes" pad />
          </div>
          <button type="button" class="btn primary" @click="open = false">OK</button>
        </div>
      </div>
    </Teleport>
  </div>
</template>
