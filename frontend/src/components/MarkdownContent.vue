<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { X } from '@lucide/vue'
import { renderMarkdown } from '@/lib/markdown'
import type { CommonRule } from '@/types/elo'
import { splitRuleTitle } from '@/lib/ruleTitle'
import ImageViewer, {
  type ImageViewerItem,
} from '@/components/ImageViewer.vue'
import { Button } from '@/components/ui/button'

const props = defineProps<{
  source?: string | null
  /** Règles disponibles pour les liens `[[slug]]` (hover / clic). */
  rules?: CommonRule[]
}>()

const root = ref<HTMLElement | null>(null)
const tooltipOpen = ref(false)
const tooltipPinned = ref(false)
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

const imageViewerOpen = ref(false)
const imageViewerIndex = ref(0)
const imageViewerItems = ref<ImageViewerItem[]>([])

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

function closeTooltip() {
  clearTimers()
  tooltipOpen.value = false
  tooltipPinned.value = false
  tooltipSlug.value = null
  activeAnchor = null
}

function showTooltip(anchor: HTMLElement, slug: string, pinned: boolean) {
  clearTimers()
  activeAnchor = anchor
  tooltipSlug.value = slug
  tooltipPinned.value = pinned
  positionTooltip(anchor)
  tooltipOpen.value = true
}

function openTooltip(anchor: HTMLElement, slug: string) {
  if (tooltipPinned.value) return
  clearTimers()
  activeAnchor = anchor
  // Même slug, autre occurrence : repositionne tout de suite sur la bonne ancre.
  if (tooltipOpen.value && tooltipSlug.value === slug) {
    positionTooltip(anchor)
    return
  }
  openTimer = setTimeout(() => {
    if (activeAnchor !== anchor || tooltipPinned.value) return
    tooltipSlug.value = slug
    positionTooltip(anchor)
    tooltipOpen.value = true
  }, 180)
}

function pinTooltip(anchor: HTMLElement, slug: string) {
  showTooltip(anchor, slug, true)
}

function scheduleClose() {
  if (tooltipPinned.value) return
  clearTimers()
  closeTimer = setTimeout(() => {
    if (tooltipPinned.value) return
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

function onRootClick(event: MouseEvent) {
  const target = event.target
  if (target instanceof Element) {
    const refEl = target.closest('.md-rule-ref')
    if (refEl instanceof HTMLElement) {
      event.preventDefault()
      event.stopPropagation()
      const slug = refEl.dataset.ruleSlug
      if (!slug) return
      if (
        tooltipPinned.value &&
        tooltipSlug.value === slug &&
        activeAnchor === refEl
      ) {
        closeTooltip()
        return
      }
      pinTooltip(refEl, slug)
      return
    }
  }
  if (!(target instanceof HTMLImageElement)) return
  if (!target.classList.contains('md-img')) return
  event.preventDefault()
  event.stopPropagation()
  const rootEl = root.value
  if (!rootEl) return
  const images = [
    ...rootEl.querySelectorAll<HTMLImageElement>('img.md-img'),
  ]
  if (images.length === 0) return
  imageViewerItems.value = images.map((img) => ({
    src: img.currentSrc || img.src,
    alt: img.alt || undefined,
    caption: img.alt || undefined,
  }))
  imageViewerIndex.value = Math.max(0, images.indexOf(target))
  imageViewerOpen.value = true
}

function onTooltipEnter() {
  clearTimers()
}

function onTooltipLeave() {
  scheduleClose()
}

function onDocumentPointerDown(event: PointerEvent) {
  if (!tooltipPinned.value) return
  const target = event.target
  if (!(target instanceof Element)) return
  if (target.closest('.md-rule-tooltip')) return
  const refEl = target.closest('.md-rule-ref')
  if (refEl && root.value?.contains(refEl)) return
  closeTooltip()
}

function onKeyDown(event: KeyboardEvent) {
  if (event.key !== 'Escape' || !tooltipOpen.value) return
  event.preventDefault()
  closeTooltip()
}

function onScrollOrResize() {
  if (!tooltipOpen.value || !activeAnchor) return
  if (!document.contains(activeAnchor)) {
    closeTooltip()
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
  window.addEventListener('keydown', onKeyDown)
  document.addEventListener('pointerdown', onDocumentPointerDown, true)
}

onBeforeUnmount(() => {
  clearTimers()
  window.removeEventListener('scroll', onScrollOrResize, true)
  window.removeEventListener('resize', onScrollOrResize)
  window.removeEventListener('keydown', onKeyDown)
  document.removeEventListener('pointerdown', onDocumentPointerDown, true)
})
</script>

<template>
  <div
    v-if="html"
    ref="root"
    class="md-content"
    @pointerover="onRootPointerOver"
    @pointerout="onRootPointerOut"
    @click="onRootClick"
    v-html="html"
  />

  <ImageViewer
    v-model:open="imageViewerOpen"
    v-model:index="imageViewerIndex"
    :items="imageViewerItems"
  />

  <Teleport to="body">
    <div
      v-if="tooltipOpen && tooltipSlug"
      class="md-rule-tooltip neon-panel"
      :style="tooltipStyle"
      @pointerenter="onTooltipEnter"
      @pointerleave="onTooltipLeave"
    >
      <div class="md-rule-tooltip-header">
        <p v-if="activeTitle" class="md-rule-tooltip-title">
          <span class="text-primary">{{ activeTitle.label }}</span>
          <span v-if="activeTitle.suffix" class="text-foreground">{{
            activeTitle.suffix
          }}</span>
        </p>
        <Button
          v-if="tooltipPinned"
          type="button"
          variant="ghost"
          size="icon-xs"
          class="md-rule-tooltip-close"
          aria-label="Fermer"
          @click.stop="closeTooltip"
        >
          <X class="size-3.5" />
        </Button>
      </div>
      <div class="md-content md-rule-tooltip-body" v-html="tooltipBodyHtml" />
    </div>
  </Teleport>
</template>
