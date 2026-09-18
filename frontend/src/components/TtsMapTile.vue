<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'
import { withBase } from '@/lib/basePath'
import type { TtsMapSummary } from '@/types/elo'

const props = defineProps<{
  map: TtsMapSummary
}>()

const pictures = computed(() => props.map.pictures ?? [])
const index = ref(0)
let timer: ReturnType<typeof setInterval> | null = null

const current = computed(() => pictures.value[index.value] ?? pictures.value[0] ?? null)

function randomIndex() {
  if (pictures.value.length <= 1) return 0
  return Math.floor(Math.random() * pictures.value.length)
}

function stop() {
  if (timer == null) return
  clearInterval(timer)
  timer = null
}

function start() {
  stop()
  if (pictures.value.length <= 1) return
  timer = setInterval(() => {
    index.value = (index.value + 1) % pictures.value.length
  }, 4000)
}

onMounted(() => {
  index.value = randomIndex()
  start()
})

onBeforeUnmount(stop)

watch(
  () => pictures.value.map((picture) => picture.id).join(','),
  () => {
    index.value = randomIndex()
    start()
  },
)
</script>

<template>
  <RouterLink
    :to="{ name: 'ressources', query: { map: map.slug } }"
    class="tts-map-tile neon-panel"
    @mouseenter="stop"
    @mouseleave="start"
  >
    <div class="tts-map-tile-media">
      <img
        v-if="current"
        :src="withBase(current.url)"
        :alt="current.original_name || map.name"
      />
      <p v-else class="tts-map-tile-empty">Pas d’image</p>
    </div>
    <p class="tts-map-tile-name">{{ map.name }}</p>
  </RouterLink>
</template>
