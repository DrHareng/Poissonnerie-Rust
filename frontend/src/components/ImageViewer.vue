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

const props = withDefaults(
  defineProps<{
    items: ImageViewerItem[]
    /** Affiche Valider / Annuler (sélection de figurine, etc.). */
    confirmable?: boolean
  }>(),
  { confirmable: false },
)

const emit = defineEmits<{
  confirm: [index: number]
  cancel: []
}>()

const safeIndex = computed(() => {
  const length = props.items.length
  if (length <= 0) return 0
  return ((index.value % length) + length) % length
})

const current = computed(() => {
  if (!props.items.length) return null
  return props.items[safeIndex.value] ?? null
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
  if (props.confirmable) emit('cancel')
  open.value = false
}

function confirm() {
  emit('confirm', safeIndex.value)
  open.value = false
}

function go(delta: number, event?: Event) {
  event?.preventDefault()
  event?.stopPropagation()
  if (!canNavigate.value) return
  const length = props.items.length
  index.value = (safeIndex.value + delta + length) % length
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
        @click.stop="close"
      >
        <X class="size-5" />
      </Button>

      <button
        v-if="canNavigate"
        type="button"
        class="image-viewer-nav image-viewer-nav--prev"
        aria-label="Image précédente"
        @click="go(-1, $event)"
      >
        <ChevronLeft class="pointer-events-none size-12" />
      </button>

      <figure class="image-viewer-figure pointer-events-none">
        <img
          :src="current.src"
          :alt="current.alt || current.caption || ''"
          class="image-viewer-image pointer-events-none"
        >
        <figcaption
          v-if="current.caption"
          class="image-viewer-caption"
        >
          {{ current.caption }}
          <span
            v-if="canNavigate"
            class="image-viewer-counter"
          >
            {{ safeIndex + 1 }} / {{ items.length }}
          </span>
        </figcaption>
      </figure>

      <button
        v-if="canNavigate"
        type="button"
        class="image-viewer-nav image-viewer-nav--next"
        aria-label="Image suivante"
        @click="go(1, $event)"
      >
        <ChevronRight class="pointer-events-none size-12" />
      </button>

      <div
        v-if="confirmable"
        class="image-viewer-actions"
        @click.stop
      >
        <Button type="button" variant="secondary" @click="close">
          Annuler
        </Button>
        <Button type="button" @click="confirm">
          Valider
        </Button>
      </div>
    </div>
  </Teleport>
</template>
