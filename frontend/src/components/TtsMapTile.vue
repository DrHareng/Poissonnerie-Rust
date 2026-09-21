<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'
import { withBase } from '@/lib/basePath'
import type { TtsMapSummary } from '@/types/elo'

const FADE_MS = 500
const DISPLAY_MS = 7500

const props = defineProps<{
  map: TtsMapSummary
}>()

const rootEl = ref<HTMLElement | null>(null)
const visible = ref(false)
const pictures = computed(() => props.map.pictures ?? [])
const index = ref(0)
const blackout = ref(false)
const panAxis = ref<'x' | 'y' | 'zoom' | null>(null)
const reversePan = ref(false)
let timer: ReturnType<typeof setTimeout> | null = null
let observer: IntersectionObserver | null = null

const current = computed(() => pictures.value[index.value] ?? pictures.value[0] ?? null)
const currentSrc = computed(() => {
  if (!current.value) return undefined
  return withBase(current.value.thumb_url || current.value.url)
})

const photoClass = computed(() => {
  if (!panAxis.value) return undefined
  return [
    `tts-map-tile-photo--${panAxis.value}`,
    reversePan.value ? 'tts-map-tile-photo--reverse' : undefined,
  ]
})

function randomIndex() {
  if (pictures.value.length <= 1) return 0
  return Math.floor(Math.random() * pictures.value.length)
}

function stopTimer() {
  if (timer == null) return
  clearTimeout(timer)
  timer = null
}

function stop() {
  stopTimer()
  blackout.value = false
}

function wait(ms: number, fn: () => void) {
  stopTimer()
  timer = setTimeout(fn, ms)
}

function start() {
  stop()
  if (!visible.value || pictures.value.length <= 1) return
  wait(DISPLAY_MS, goBlack)
}

function goBlack() {
  if (!visible.value) return
  blackout.value = true
  wait(FADE_MS, swapPhoto)
}

function swapPhoto() {
  if (!visible.value || pictures.value.length === 0) return
  index.value = (index.value + 1) % pictures.value.length
  panAxis.value = null
  blackout.value = false
  wait(DISPLAY_MS, goBlack)
}

function onPhotoLoad(event: Event) {
  const img = event.target as HTMLImageElement
  const width = img.naturalWidth
  const height = img.naturalHeight
  if (width <= 0 || height <= 0) return
  const ratio = width / height
  if (ratio >= 1.08) panAxis.value = 'x'
  else if (ratio <= 0.92) panAxis.value = 'y'
  else panAxis.value = 'zoom'
  reversePan.value = Math.random() < 0.5
}

function setVisible(next: boolean) {
  if (visible.value === next) {
    if (next) start()
    return
  }
  visible.value = next
  if (next) start()
  else stop()
}

onMounted(() => {
  index.value = randomIndex()
  if (typeof IntersectionObserver === 'undefined') {
    setVisible(true)
    return
  }
  observer = new IntersectionObserver(
    ([entry]) => {
      setVisible(Boolean(entry?.isIntersecting))
    },
    { rootMargin: '240px 0px', threshold: 0.01 },
  )
  if (rootEl.value) observer.observe(rootEl.value)
})

onBeforeUnmount(() => {
  observer?.disconnect()
  observer = null
  stop()
})

watch(
  () => pictures.value.map((picture) => picture.id).join(','),
  () => {
    index.value = randomIndex()
    panAxis.value = null
    start()
  },
)
</script>

<template>
  <RouterLink
    :to="{ name: 'maps', query: { map: map.slug } }"
    class="tts-map-tile neon-panel"
    @mouseenter="stop"
    @mouseleave="start"
  >
    <div ref="rootEl" class="tts-map-tile-media">
      <img
        v-if="visible && currentSrc"
        :key="current?.id"
        class="tts-map-tile-photo"
        :class="photoClass"
        :src="currentSrc"
        :alt="current?.original_name || map.name"
        loading="lazy"
        decoding="async"
        @load="onPhotoLoad"
      />
      <p v-else-if="!current" class="tts-map-tile-empty">Pas d’image</p>
      <div
        class="tts-map-tile-fade"
        :class="{ 'tts-map-tile-fade--on': blackout }"
      />
    </div>
    <p class="tts-map-tile-name">{{ map.name }}</p>
  </RouterLink>
</template>
