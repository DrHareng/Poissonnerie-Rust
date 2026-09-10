<script setup lang="ts">
import type { SecondaryObjective } from '@/types/elo'
import { secondaryImageSrc } from '@/lib/secondaryImages'

const props = defineProps<{
  secondaries: SecondaryObjective[]
  selectable?: boolean
  viewable?: boolean
  choosable?: boolean
  selectedSlug?: string
  /** Affichage grisé + croix rouge (bannis). */
  banned?: boolean
  compact?: boolean
}>()

const emit = defineEmits<{
  select: [slug: string]
  view: [slug: string]
  choose: [slug: string]
}>()

function isClickable(slug: string) {
  if (props.banned) return false
  if (props.choosable) return true
  if (props.selectable || props.viewable) return !!secondaryImageSrc(slug)
  return false
}

function onImageClick(slug: string) {
  if (props.choosable) {
    emit('choose', slug)
    return
  }
  if (props.selectable) {
    emit('select', slug)
    return
  }
  if (props.viewable && secondaryImageSrc(slug)) {
    emit('view', slug)
  }
}
</script>

<template>
  <div
    class="secondary-card-grid"
    :class="{ 'secondary-card-grid--compact': compact }"
  >
    <div
      v-for="secondary in secondaries"
      :key="secondary.id"
      class="neon-panel secondary-card relative shrink-0 overflow-hidden p-0"
      :class="{
        'secondary-card--selected':
          (choosable || selectable) && selectedSlug === secondary.slug,
        'secondary-card--banned': banned,
      }"
    >
      <span
        v-if="banned"
        class="secondary-card-ban-mark"
        aria-hidden="true"
      >
        <svg viewBox="0 0 100 100" preserveAspectRatio="none">
          <line x1="0" y1="0" x2="100" y2="100" />
          <line x1="100" y1="0" x2="0" y2="100" />
        </svg>
      </span>

      <button
        type="button"
        class="block w-full p-0 text-left"
        :class="{
          'secondary-card--clickable': isClickable(secondary.slug),
        }"
        :disabled="!isClickable(secondary.slug)"
        :aria-pressed="
          choosable ? selectedSlug === secondary.slug : undefined
        "
        :title="choosable ? `Choisir ${secondary.name}` : undefined"
        @click="onImageClick(secondary.slug)"
      >
        <img
          v-if="secondaryImageSrc(secondary.slug)"
          :src="secondaryImageSrc(secondary.slug)"
          :alt="secondary.name"
          class="secondary-card-image"
        />
        <div v-else class="p-3 text-sm font-medium">{{ secondary.name }}</div>
      </button>
    </div>
  </div>
</template>
