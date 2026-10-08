<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { onKeyStroke, useScrollLock } from '@vueuse/core'
import { Info, X } from '@lucide/vue'
import type { ScenarioContentUpdate } from '@/types/elo'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

const props = defineProps<{
  slug: string
  name: string
  active: boolean
  updates: ScenarioContentUpdate[]
}>()

const emit = defineEmits<{
  select: [event: MouseEvent]
}>()

const dialogOpen = ref(false)
const tipOpen = ref(false)
const tipStyle = ref<Record<string, string>>({})
const trigger = ref<HTMLButtonElement | null>(null)

let openTimer: ReturnType<typeof setTimeout> | null = null
let closeTimer: ReturnType<typeof setTimeout> | null = null

const latest = computed(() => props.updates[0] ?? null)

const body = typeof document !== 'undefined' ? document.body : null
const isLocked = useScrollLock(body)

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

function openDialog(event: MouseEvent) {
  event.preventDefault()
  event.stopPropagation()
  hideTip()
  dialogOpen.value = true
  isLocked.value = true
}

function closeDialog() {
  dialogOpen.value = false
  isLocked.value = false
}

function onBackdropClick(event: MouseEvent) {
  if (event.target === event.currentTarget) closeDialog()
}

onKeyStroke('Escape', (event) => {
  if (!dialogOpen.value) return
  event.preventDefault()
  closeDialog()
})

if (typeof window !== 'undefined') {
  window.addEventListener('scroll', onScrollOrResize, true)
  window.addEventListener('resize', onScrollOrResize)
}

function onScrollOrResize() {
  if (!tipOpen.value) return
  positionTip()
}

onBeforeUnmount(() => {
  isLocked.value = false
  clearTipTimers()
  window.removeEventListener('scroll', onScrollOrResize, true)
  window.removeEventListener('resize', onScrollOrResize)
})
</script>

<template>
  <div
    class="scenario-side-item"
    :class="{ 'scenario-side-item--active': active }"
  >
    <RouterLink
      :to="{ name: 'scenarios', query: { scenario: slug } }"
      class="scenario-side-item-link"
      :aria-current="active ? 'page' : undefined"
      @click="emit('select', $event)"
    >
      {{ name }}
    </RouterLink>
    <button
      v-if="latest"
      ref="trigger"
      type="button"
      class="content-update-info"
      :aria-label="`Mises à jour de ${name}`"
      @pointerenter="scheduleTip"
      @pointerleave="scheduleTipClose"
      @focus="scheduleTip"
      @blur="scheduleTipClose"
      @click="openDialog"
    >
      <Info class="size-3" />
    </button>
  </div>

  <Teleport to="body">
    <div
      v-if="tipOpen && latest"
      class="md-rule-tooltip neon-panel pointer-events-auto"
      :style="tipStyle"
      role="tooltip"
      @pointerenter="clearTipTimers"
      @pointerleave="scheduleTipClose"
    >
      <time
        class="mb-1 block text-xs font-medium text-primary"
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
      :aria-labelledby="`scenario-updates-${slug}`"
      @click="onBackdropClick"
    >
      <Card class="player-detail-modal neon-panel w-full max-w-lg">
        <CardHeader class="relative pr-12">
          <CardTitle :id="`scenario-updates-${slug}`">{{ name }}</CardTitle>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            class="absolute top-3 right-3"
            aria-label="Fermer"
            @click="closeDialog"
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
              <time
                class="text-xs font-medium text-primary"
                :datetime="new Date(update.created_at * 1000).toISOString()"
              >
                {{ formatDateTime(update.created_at) }}
              </time>
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
