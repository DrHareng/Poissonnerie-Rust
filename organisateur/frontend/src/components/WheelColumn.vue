<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, useId, watch } from 'vue'

const ITEM = 36

const props = defineProps<{
  modelValue: number
  min: number
  max: number
  label: string
  pad?: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: number]
}>()

const labelId = useId()
const scroller = ref<HTMLElement | null>(null)
const preview = ref(props.modelValue)
let settle = 0

const values = computed(() => {
  const list: number[] = []
  for (let value = props.min; value <= props.max; value += 1) list.push(value)
  return list
})

function clamp(value: number) {
  return Math.min(props.max, Math.max(props.min, value))
}

function text(value: number) {
  const shown = String(value)
  return props.pad ? shown.padStart(2, '0') : shown
}

function indexFromScroll() {
  const el = scroller.value
  if (!el) return 0
  return Math.round(el.scrollTop / ITEM)
}

function place(value: number) {
  const el = scroller.value
  if (!el) return
  const top = (clamp(value) - props.min) * ITEM
  if (Math.abs(el.scrollTop - top) > 1) el.scrollTop = top
}

function commit() {
  const value = clamp(props.min + indexFromScroll())
  preview.value = value
  place(value)
  if (value !== props.modelValue) emit('update:modelValue', value)
}

function onScroll() {
  preview.value = clamp(props.min + indexFromScroll())
  window.clearTimeout(settle)
  settle = window.setTimeout(commit, 140)
}

function onScrollEnd() {
  window.clearTimeout(settle)
  commit()
}

function nudge(delta: number) {
  const value = clamp(props.modelValue + delta)
  preview.value = value
  place(value)
  emit('update:modelValue', value)
}

onMounted(() => {
  preview.value = clamp(props.modelValue)
  place(props.modelValue)
  scroller.value?.addEventListener('scrollend', onScrollEnd)
})

watch(
  () => props.modelValue,
  (value) => {
    preview.value = clamp(value)
    place(value)
  },
)

onBeforeUnmount(() => {
  window.clearTimeout(settle)
  scroller.value?.removeEventListener('scrollend', onScrollEnd)
})
</script>

<template>
  <div class="wheel-column">
    <span :id="labelId">{{ label }}</span>
    <div class="wheel-window">
      <div
        ref="scroller"
        class="wheel-scroller"
        tabindex="0"
        role="spinbutton"
        :aria-labelledby="labelId"
        :aria-valuemin="min"
        :aria-valuemax="max"
        :aria-valuenow="modelValue"
        :aria-valuetext="text(modelValue)"
        @scroll.passive="onScroll"
        @keydown.up.prevent="nudge(-1)"
        @keydown.down.prevent="nudge(1)"
      >
        <div class="wheel-pad" />
        <div v-for="value in values" :key="value" class="wheel-item" :class="{ current: value === preview }">
          {{ text(value) }}
        </div>
        <div class="wheel-pad" />
      </div>
    </div>
  </div>
</template>
