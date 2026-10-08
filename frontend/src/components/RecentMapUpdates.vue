<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { onKeyStroke, useScrollLock } from '@vueuse/core'
import { Info, X } from '@lucide/vue'
import { fetchRecentMapUpdates } from '@/lib/api'
import type { MapContentUpdate } from '@/types/elo'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

const props = defineProps<{
  refreshKey?: number
}>()

const emit = defineEmits<{
  visible: [value: boolean]
}>()

const updates = ref<MapContentUpdate[]>([])
const ready = ref(false)
const dialogOpen = ref(false)
const tipOpen = ref(false)
const tipStyle = ref<Record<string, string>>({})
const trigger = ref<HTMLButtonElement | null>(null)

let requestId = 0
let openTimer: ReturnType<typeof setTimeout> | null = null
let closeTimer: ReturnType<typeof setTimeout> | null = null

const latest = computed(() => updates.value[0] ?? null)
const showBadge = computed(() => ready.value && latest.value != null)

const body = typeof document !== 'undefined' ? document.body : null
const isLocked = useScrollLock(body)

watch(dialogOpen, (value) => {
  isLocked.value = value
})

watch(showBadge, (value) => emit('visible', value), { immediate: true })

watch(
  () => props.refreshKey,
  () => {
    void load(true)
  },
)

onKeyStroke('Escape', (event) => {
  if (!dialogOpen.value) return
  event.preventDefault()
  dialogOpen.value = false
})

if (typeof window !== 'undefined') {
  window.addEventListener('scroll', onScrollOrResize, true)
  window.addEventListener('resize', onScrollOrResize)
}

onBeforeUnmount(() => {
  requestId += 1
  emit('visible', false)
  isLocked.value = false
  clearTipTimers()
  window.removeEventListener('scroll', onScrollOrResize, true)
  window.removeEventListener('resize', onScrollOrResize)
})

function formatDateTime(timestamp: number) {
  return new Intl.DateTimeFormat('fr-FR', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(timestamp * 1000))
}

function clearTipTimers() {
  if (openTimer) {
    clearTimeout(openTimer)
    openTimer = null
  }
  if (closeTimer) {
    clearTimeout(closeTimer)
    closeTimer = null
  }
}

function hideTip() {
  clearTipTimers()
  tipOpen.value = false
}

function positionTip() {
  const rect = trigger.value?.getBoundingClientRect()
  if (!rect) return
  const margin = 8
  const maxWidth = 320
  const left = Math.min(
    Math.max(margin, rect.left),
    window.innerWidth - maxWidth - margin,
  )
  tipStyle.value = {
    left: `${left}px`,
    top: `${rect.bottom + margin}px`,
    maxWidth: `${maxWidth}px`,
  }
}

function scheduleTip() {
  if (!latest.value || dialogOpen.value) return
  clearTipTimers()
  openTimer = setTimeout(() => {
    positionTip()
    tipOpen.value = true
  }, 180)
}

function scheduleTipClose() {
  clearTipTimers()
  closeTimer = setTimeout(() => {
    tipOpen.value = false
  }, 120)
}

function onScrollOrResize() {
  if (!tipOpen.value) return
  positionTip()
}

function openDialog() {
  hideTip()
  dialogOpen.value = true
}

function onBackdropClick(event: MouseEvent) {
  if (event.target === event.currentTarget) dialogOpen.value = false
}

async function load(background = false) {
  const id = ++requestId
  if (!background) ready.value = false
  try {
    const items = await fetchRecentMapUpdates(30)
    if (id !== requestId) return
    updates.value = items
  } catch (error) {
    if (id !== requestId) return
    updates.value = []
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de charger les mises à jour',
    )
  } finally {
    if (id === requestId) ready.value = true
  }
}

void load()
</script>

<template>
  <button
    v-if="showBadge && latest"
    ref="trigger"
    type="button"
    class="inline-flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground"
    aria-label="Mises à jour récentes"
    @pointerenter="scheduleTip"
    @pointerleave="scheduleTipClose"
    @focus="scheduleTip"
    @blur="scheduleTipClose"
    @click="openDialog"
  >
    <span class="min-w-0 truncate">Mises à jour récentes</span>
    <span class="content-update-info" aria-hidden="true">
      <Info class="size-3" />
    </span>
  </button>

  <Teleport to="body">
    <div
      v-if="tipOpen && latest"
      class="md-rule-tooltip neon-panel pointer-events-auto"
      :style="tipStyle"
      role="tooltip"
      @pointerenter="clearTipTimers"
      @pointerleave="scheduleTipClose"
    >
      <p class="text-sm font-semibold text-primary">{{ latest.map_name }}</p>
      <time
        class="mb-1 block text-xs text-muted-foreground"
        :datetime="new Date(latest.created_at * 1000).toISOString()"
      >
        {{ formatDateTime(latest.created_at) }}
      </time>
      <p class="whitespace-pre-wrap break-words text-sm text-foreground/90">
        {{ latest.description }}
      </p>
    </div>

    <div
      v-if="dialogOpen"
      class="player-detail-overlay"
      role="dialog"
      aria-modal="true"
      aria-labelledby="recent-map-updates-title"
      @click="onBackdropClick"
    >
      <Card class="player-detail-modal neon-panel w-full max-w-lg">
        <CardHeader class="relative pr-12">
          <CardTitle id="recent-map-updates-title">Mises à jour récentes</CardTitle>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            class="absolute top-3 right-3"
            aria-label="Fermer"
            @click="dialogOpen = false"
          >
            <X class="size-4" />
          </Button>
        </CardHeader>
        <CardContent>
          <ol class="space-y-3">
            <li
              v-for="update in updates"
              :key="update.id"
              class="space-y-1"
            >
              <div class="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-0.5">
                <RouterLink
                  :to="{ name: 'maps', query: { map: update.map_slug } }"
                  class="text-sm font-medium text-primary hover:underline"
                  @click="dialogOpen = false"
                >
                  {{ update.map_name }}
                </RouterLink>
                <time
                  class="text-xs text-muted-foreground"
                  :datetime="new Date(update.created_at * 1000).toISOString()"
                >
                  {{ formatDateTime(update.created_at) }}
                </time>
              </div>
              <p class="whitespace-pre-wrap break-words text-sm text-foreground/90">
                {{ update.description }}
              </p>
            </li>
          </ol>
        </CardContent>
      </Card>
    </div>
  </Teleport>
</template>
