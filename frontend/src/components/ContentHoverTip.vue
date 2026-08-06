<script setup lang="ts">
import { computed, onBeforeUnmount, ref, useSlots } from 'vue'
import { renderMarkdown } from '@/lib/markdown'
import { cn } from '@/lib/utils'

const props = withDefaults(
  defineProps<{
    label?: string
    title: string
    bodyMd?: string | null
    class?: string
  }>(),
  { label: '' },
)

const slots = useSlots()
const hasCustomTrigger = computed(() => Boolean(slots.default))

const open = ref(false)
const style = ref<Record<string, string>>({})
const trigger = ref<HTMLElement | null>(null)
let openTimer: ReturnType<typeof setTimeout> | null = null
let closeTimer: ReturnType<typeof setTimeout> | null = null

const bodyHtml = computed(() => {
  if (!props.bodyMd?.trim()) {
    return '<p class="md-note"><em>Description indisponible.</em></p>'
  }
  return renderMarkdown(props.bodyMd)
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

function position() {
  if (!trigger.value) return
  const rect = trigger.value.getBoundingClientRect()
  const margin = 8
  const maxWidth = 360
  const left = Math.min(
    Math.max(margin, rect.left),
    window.innerWidth - maxWidth - margin,
  )
  const spaceBelow = window.innerHeight - rect.bottom
  const placeAbove = spaceBelow < 220 && rect.top > spaceBelow
  style.value = placeAbove
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

function scheduleOpen() {
  clearTimers()
  openTimer = setTimeout(() => {
    position()
    open.value = true
  }, 180)
}

function scheduleClose() {
  clearTimers()
  closeTimer = setTimeout(() => {
    open.value = false
  }, 120)
}

function onScrollOrResize() {
  if (!open.value) return
  position()
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
  <component
    :is="hasCustomTrigger ? 'div' : 'button'"
    ref="trigger"
    :type="hasCustomTrigger ? undefined : 'button'"
    :tabindex="hasCustomTrigger ? 0 : undefined"
    :class="
      hasCustomTrigger
        ? cn('cursor-help', props.class)
        : cn(
            'md-rule-ref cursor-help border-0 bg-transparent p-0 text-left font-medium text-foreground',
            props.class,
          )
    "
    @pointerenter="scheduleOpen"
    @pointerleave="scheduleClose"
    @focus="scheduleOpen"
    @blur="scheduleClose"
  >
    <slot>{{ label }}</slot>
  </component>

  <Teleport to="body">
    <div
      v-if="open"
      class="md-rule-tooltip neon-panel"
      :style="style"
      @pointerenter="clearTimers"
      @pointerleave="scheduleClose"
    >
      <p class="md-rule-tooltip-title">
        <span class="text-primary">{{ title }}</span>
      </p>
      <div class="md-content md-rule-tooltip-body" v-html="bodyHtml" />
    </div>
  </Teleport>
</template>
