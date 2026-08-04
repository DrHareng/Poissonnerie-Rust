<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { renderMarkdown } from '@/lib/markdown'
import type { CommonRule } from '@/types/elo'
import { splitRuleTitle } from '@/lib/ruleTitle'

const props = defineProps<{
  source?: string | null
  /** Règles disponibles pour les liens `[[slug]]` (hover). */
  rules?: CommonRule[]
}>()

const root = ref<HTMLElement | null>(null)
const tooltipOpen = ref(false)
const tooltipSlug = ref<string | null>(null)
const tooltipStyle = ref<Record<string, string>>({})
let activeAnchor: HTMLElement | null = null
let openTimer: ReturnType<typeof setTimeout> | null = null
let closeTimer: ReturnType<typeof setTimeout> | null = null

const rulesBySlug = computed(() => {
  const map = new Map<string, CommonRule>()
  for (const rule of props.rules ?? []) {
    map.set(rule.slug, rule)
  }
  return map
})

const html = computed(() => {
  if (!props.source?.trim()) return ''
  return renderMarkdown(props.source, {
    ruleLabel: (slug) => rulesBySlug.value.get(slug)?.name ?? slug,
  })
})

const activeRule = computed(() =>
  tooltipSlug.value ? rulesBySlug.value.get(tooltipSlug.value) ?? null : null,
)

const activeTitle = computed(() => {
  if (!activeRule.value) {
    return tooltipSlug.value
      ? { label: tooltipSlug.value, suffix: null }
      : null
  }
  return splitRuleTitle(activeRule.value.name)
})

const tooltipBodyHtml = computed(() => {
  if (!activeRule.value) {
    return '<p class="md-note"><em>Règle introuvable sur ce scénario.</em></p>'
  }
  return renderMarkdown(activeRule.value.body_md)
})

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

function positionTooltip(anchor: HTMLElement) {
  const rect = anchor.getBoundingClientRect()
  const margin = 8
  const maxWidth = 360
  const left = Math.min(
    Math.max(margin, rect.left),
    window.innerWidth - maxWidth - margin,
  )
  const spaceBelow = window.innerHeight - rect.bottom
  const placeAbove = spaceBelow < 220 && rect.top > spaceBelow
  tooltipStyle.value = placeAbove
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

function openTooltip(anchor: HTMLElement, slug: string) {
  clearTimers()
  activeAnchor = anchor
  // Même slug, autre occurrence : repositionne tout de suite sur la bonne ancre.
  if (tooltipOpen.value && tooltipSlug.value === slug) {
    positionTooltip(anchor)
    return
  }
  openTimer = setTimeout(() => {
    if (activeAnchor !== anchor) return
    tooltipSlug.value = slug
    positionTooltip(anchor)
    tooltipOpen.value = true
  }, 180)
}

function scheduleClose() {
  clearTimers()
  closeTimer = setTimeout(() => {
    tooltipOpen.value = false
    tooltipSlug.value = null
    activeAnchor = null
  }, 120)
}

function onRootPointerOver(event: PointerEvent) {
  const target = event.target
  if (!(target instanceof Element)) return
  const refEl = target.closest('.md-rule-ref')
  if (!(refEl instanceof HTMLElement)) return
  const slug = refEl.dataset.ruleSlug
  if (!slug) return
  openTooltip(refEl, slug)
}

function onRootPointerOut(event: PointerEvent) {
  const related = event.relatedTarget
  if (related instanceof Node && root.value?.contains(related)) {
    const stillOnRef =
      related instanceof Element && related.closest('.md-rule-ref')
    if (stillOnRef) return
  }
  // Ne ferme pas si on passe sur le tooltip lui-même.
  if (related instanceof Element && related.closest('.md-rule-tooltip')) {
    return
  }
  scheduleClose()
}

function onTooltipEnter() {
  clearTimers()
}

function onTooltipLeave() {
  scheduleClose()
}

function onScrollOrResize() {
  if (!tooltipOpen.value || !activeAnchor) return
  if (!document.contains(activeAnchor)) {
    scheduleClose()
    return
  }
  positionTooltip(activeAnchor)
}

watch(html, async () => {
  await nextTick()
  onScrollOrResize()
})

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
  <div
    v-if="html"
    ref="root"
    class="md-content"
    @pointerover="onRootPointerOver"
    @pointerout="onRootPointerOut"
    v-html="html"
  />

  <Teleport to="body">
    <div
      v-if="tooltipOpen && tooltipSlug"
      class="md-rule-tooltip neon-panel"
      :style="tooltipStyle"
      @pointerenter="onTooltipEnter"
      @pointerleave="onTooltipLeave"
    >
      <p v-if="activeTitle" class="md-rule-tooltip-title">
        <span class="text-primary">{{ activeTitle.label }}</span>
        <span v-if="activeTitle.suffix" class="text-foreground">{{
          activeTitle.suffix
        }}</span>
      </p>
      <div class="md-content md-rule-tooltip-body" v-html="tooltipBodyHtml" />
    </div>
  </Teleport>
</template>
