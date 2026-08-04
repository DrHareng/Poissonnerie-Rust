<script setup lang="ts">
import { computed, onUnmounted, watch } from 'vue'
import { onKeyStroke, useScrollLock } from '@vueuse/core'
import { ChevronLeft, ChevronRight, X } from '@lucide/vue'
import { Button } from '@/components/ui/button'

export interface ImageViewerItem {
  src: string
  alt?: string
  caption?: string
}

const open = defineModel<boolean>('open', { default: false })
const index = defineModel<number>('index', { default: 0 })

const props = defineProps<{
  items: ImageViewerItem[]
}>()

const current = computed(() => {
  if (!props.items.length) return null
  const i = ((index.value % props.items.length) + props.items.length) % props.items.length
  return props.items[i] ?? null
})

const canNavigate = computed(() => props.items.length > 1)

const body = typeof document !== 'undefined' ? document.body : null
const isLocked = useScrollLock(body)
watch(
  open,
  (value) => {
    isLocked.value = value
  },
  { immediate: true },
)
onUnmounted(() => {
  isLocked.value = false
})

watch(
  () => props.items.length,
  (length) => {
    if (length === 0) {
      open.value = false
      return
    }
    if (index.value >= length) {
      index.value = length - 1
    }
  },
)

function close() {
  open.value = false
}

function go(delta: number) {
  if (!canNavigate.value) return
  const length = props.items.length
  index.value = (index.value + delta + length) % length
}

function onBackdropClick(event: MouseEvent) {
  if (event.target === event.currentTarget) {
    close()
  }
}

onKeyStroke('Escape', (event) => {
  if (!open.value) return
  event.preventDefault()
  close()
})

onKeyStroke('ArrowLeft', (event) => {
  if (!open.value) return
  event.preventDefault()
  go(-1)
})

onKeyStroke('ArrowRight', (event) => {
  if (!open.value) return
  event.preventDefault()
  go(1)
})
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open && current"
      class="image-viewer-overlay"
      role="dialog"
      aria-modal="true"
      :aria-label="current.caption || current.alt || 'Visualiseur d\'image'"
      @click="onBackdropClick"
    >
      <Button
        type="button"
        variant="ghost"
        size="icon"
        class="image-viewer-close"
        aria-label="Fermer"
        @click="close"
      >
        <X class="size-5" />
      </Button>

      <Button
        v-if="canNavigate"
        type="button"
        variant="ghost"
        size="icon"
        class="image-viewer-nav image-viewer-nav--prev"
        aria-label="Image précédente"
        @click="go(-1)"
      >
        <ChevronLeft class="size-12" />
      </Button>

      <figure class="image-viewer-figure">
        <img
          :src="current.src"
          :alt="current.alt || current.caption || ''"
          class="image-viewer-image"
        />
        <figcaption
          v-if="current.caption"
          class="image-viewer-caption"
        >
          {{ current.caption }}
          <span
            v-if="canNavigate"
            class="image-viewer-counter"
          >
            {{ index + 1 }} / {{ items.length }}
          </span>
        </figcaption>
      </figure>

      <Button
        v-if="canNavigate"
        type="button"
        variant="ghost"
        size="icon"
        class="image-viewer-nav image-viewer-nav--next"
        aria-label="Image suivante"
        @click="go(1)"
      >
        <ChevronRight class="size-12" />
      </Button>
    </div>
  </Teleport>
</template>
