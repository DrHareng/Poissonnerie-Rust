<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { armyLogoUrl } from '@/format'
import type { Sectorial } from '@/types'

const props = defineProps<{
  sectorials: Sectorial[]
  modelValue: number
}>()

const emit = defineEmits<{
  'update:modelValue': [id: number]
}>()

const open = ref(false)
const root = ref<HTMLElement | null>(null)

const selected = computed(() => props.sectorials.find((item) => item.id === props.modelValue) ?? null)

function logo(item: Sectorial) {
  return armyLogoUrl(item.slug)
}

function choose(id: number) {
  emit('update:modelValue', id)
  open.value = false
}

function onPointerDown(event: PointerEvent) {
  if (!root.value?.contains(event.target as Node)) open.value = false
}

onMounted(() => document.addEventListener('pointerdown', onPointerDown))
onBeforeUnmount(() => document.removeEventListener('pointerdown', onPointerDown))
</script>

<template>
  <div ref="root" class="picker">
    <button type="button" class="picker-value" @click="open = !open">
      <img v-if="selected" class="army-logo" :src="logo(selected)" alt="" />
      <span class="clip">{{ selected ? selected.name : 'Choisir' }}</span>
    </button>
    <div v-if="open" class="picker-menu">
      <button
        v-for="item in sectorials"
        :key="item.id"
        type="button"
        class="picker-option"
        :class="{ current: item.id === modelValue }"
        :title="item.name"
        @click="choose(item.id)"
      >
        <img class="army-logo" :src="logo(item)" alt="" />
        <span class="clip">{{ item.name }}</span>
      </button>
    </div>
  </div>
</template>
