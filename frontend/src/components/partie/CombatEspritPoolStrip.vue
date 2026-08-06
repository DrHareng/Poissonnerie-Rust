<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { secondaryImageSrc } from '@/lib/secondaryImages'
import { renderMarkdown } from '@/lib/markdown'
import type { PoolSlotStatus } from '@/lib/combatEspritDraft'

export type CombatEspritStripItem = {
  slug: string
  name?: string
  bodyMd?: string | null
  status: PoolSlotStatus
  owner?: 'player1' | 'player2' | null
}

const props = defineProps<{
  items: CombatEspritStripItem[]
  selectable?: boolean
  /** Si vrai, les cartes prises laissent un emplacement vide (draft). */
  hideTaken?: boolean
}>()

const emit = defineEmits<{
  select: [slug: string]
}>()

const openSlug = ref<string | null>(null)
const tipStyle = ref<Record<string, string>>({})
const triggers = ref<Record<string, HTMLElement | null>>({})
let openTimer: ReturnType<typeof setTimeout> | null = null
let closeTimer: ReturnType<typeof setTimeout> | null = null

const openItem = computed(() =>
  props.items.find((item) => item.slug === openSlug.value) ?? null,
)

const tipHtml = computed(() => {
  const body = openItem.value?.bodyMd?.trim()
  if (!body) {
    return '<p class="md-note"><em>Description indisponible.</em></p>'
  }
  return renderMarkdown(body)
})

function setTrigger(slug: string, el: unknown) {
  triggers.value[slug] = (el as HTMLElement | null) ?? null
}

function clearTimers() {
  if (openTimer) {
    clearTimeout(openTimer)
    openTimer = null
  }
  if (closeTimer) {
    clearTimeout(closeTimer)
    closeTimer = null
  }
}

function position(slug: string) {
  const trigger = triggers.value[slug]
  if (!trigger) return
  const rect = trigger.getBoundingClientRect()
  const margin = 8
  const maxWidth = 360
  const left = Math.min(
    Math.max(margin, rect.left),
    window.innerWidth - maxWidth - margin,
  )
  const spaceBelow = window.innerHeight - rect.bottom
  const placeAbove = spaceBelow < 220 && rect.top > spaceBelow
  tipStyle.value = placeAbove
    ? {
        left: `${left}px`,
        bottom: `${window.innerHeight - rect.top + margin}px`,
        top: 'auto',
        maxWidth: `${maxWidth}px`,
      }
    : {
        left: `${left}px`,
        top: `${rect.bottom + margin}px`,
        bottom: 'auto',
        maxWidth: `${maxWidth}px`,
      }
}

function scheduleOpen(slug: string) {
  clearTimers()
  openTimer = setTimeout(() => {
    position(slug)
    openSlug.value = slug
  }, 180)
}

function scheduleClose() {
  clearTimers()
  closeTimer = setTimeout(() => {
    openSlug.value = null
  }, 120)
}

function onClick(item: CombatEspritStripItem) {
  if (props.selectable && item.status === 'available') {
    emit('select', item.slug)
  }
}

function onScrollOrResize() {
  if (!openSlug.value) return
  position(openSlug.value)
}

if (typeof window !== 'undefined') {
  window.addEventListener('scroll', onScrollOrResize, true)
  window.addEventListener('resize', onScrollOrResize)
}

onBeforeUnmount(() => {
  clearTimers()
  window.removeEventListener('scroll', onScrollOrResize, true)
  window.removeEventListener('resize', onScrollOrResize)
})
</script>

<template>
  <div class="combat-esprit-strip" role="list">
    <button
      v-for="(item, index) in items"
      :key="`${item.slug}-${index}`"
      :ref="(el) => setTrigger(item.slug, el)"
      type="button"
      role="listitem"
      class="combat-esprit-strip-slot neon-panel"
      :class="{
        'combat-esprit-strip-slot--available': item.status === 'available',
        'combat-esprit-strip-slot--banned': !hideTaken && item.status === 'banned',
        'combat-esprit-strip-slot--taken': item.status === 'taken',
        'combat-esprit-strip-slot--empty':
          hideTaken && (item.status === 'taken' || item.status === 'banned'),
        'combat-esprit-strip-slot--taken-p1':
          !hideTaken && item.status === 'taken' && item.owner === 'player1',
        'combat-esprit-strip-slot--taken-p2':
          !hideTaken && item.status === 'taken' && item.owner === 'player2',
        'combat-esprit-strip-slot--selectable':
          selectable && item.status === 'available',
      }"
      :disabled="selectable && item.status !== 'available'"
      :aria-label="item.name ?? item.slug"
      @click="onClick(item)"
      @pointerenter="scheduleOpen(item.slug)"
      @pointerleave="scheduleClose"
      @focus="scheduleOpen(item.slug)"
      @blur="scheduleClose"
    >
      <template
        v-if="!(hideTaken && (item.status === 'taken' || item.status === 'banned'))"
      >
        <img
          v-if="secondaryImageSrc(item.slug)"
          :src="secondaryImageSrc(item.slug)"
          :alt="item.name ?? item.slug"
          class="combat-esprit-strip-image"
        />
        <span v-else class="combat-esprit-strip-fallback">{{
          item.name ?? item.slug
        }}</span>
        <span
          v-if="item.status === 'banned'"
          class="combat-esprit-strip-ban"
          aria-hidden="true"
        >
          <svg viewBox="0 0 100 100" preserveAspectRatio="none">
            <line x1="0" y1="0" x2="100" y2="100" />
            <line x1="100" y1="0" x2="0" y2="100" />
          </svg>
        </span>
      </template>
    </button>
  </div>

  <Teleport to="body">
    <div
      v-if="openItem"
      class="md-rule-tooltip neon-panel"
      :style="tipStyle"
      @pointerenter="clearTimers"
      @pointerleave="scheduleClose"
    >
      <p class="md-rule-tooltip-title">
        <span class="text-primary">{{ openItem.name ?? openItem.slug }}</span>
      </p>
      <div class="md-content md-rule-tooltip-body" v-html="tipHtml" />
    </div>
  </Teleport>
</template>
