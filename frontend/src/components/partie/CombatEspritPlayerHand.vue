<script setup lang="ts">
import { computed } from 'vue'
import type { SecondaryObjective } from '@/types/elo'
import ContentHoverTip from '@/components/ContentHoverTip.vue'
import { secondaryImageSrc } from '@/lib/secondaryImages'
import type { draftStepBadges } from '@/lib/combatEspritDraft'

type StepBadge = ReturnType<typeof draftStepBadges>[number]

const props = withDefaults(
  defineProps<{
    ban: SecondaryObjective | null
    picks: Array<SecondaryObjective | null>
    badges?: StepBadge[]
    currentStepNumber?: number | null
    compact?: boolean
  }>(),
  {
    badges: () => [],
    currentStepNumber: null,
    compact: false,
  },
)

const pickSlots = computed(() =>
  [0, 1, 2].map((index) => props.picks[index] ?? null),
)
</script>

<template>
  <div
    class="combat-esprit-hand-slots"
    :class="{ 'combat-esprit-hand-slots--compact': compact }"
  >
    <div
      class="combat-esprit-hand-slot combat-esprit-hand-slot--ban"
      :class="{ 'combat-esprit-hand-slot--filled': !!ban }"
      style="grid-column: 1; grid-row: 1"
    >
      <ContentHoverTip
        v-if="ban"
        class="absolute inset-0 block"
        :title="ban.name"
        :body-md="ban.body_md"
      >
        <img
          v-if="secondaryImageSrc(ban.slug)"
          :src="secondaryImageSrc(ban.slug)"
          :alt="ban.name"
          class="combat-esprit-hand-slot-image combat-esprit-hand-slot-image--banned"
        />
        <span v-else class="combat-esprit-hand-slot-fallback">{{ ban.name }}</span>
      </ContentHoverTip>
      <span class="combat-esprit-ban-card-mark" aria-hidden="true">
        <svg viewBox="0 0 100 100" preserveAspectRatio="none">
          <line x1="0" y1="0" x2="100" y2="100" />
          <line x1="100" y1="0" x2="0" y2="100" />
        </svg>
      </span>
    </div>
    <div
      v-for="(card, index) in pickSlots"
      :key="`pick-${index}`"
      class="combat-esprit-hand-slot"
      :class="{ 'combat-esprit-hand-slot--filled': !!card }"
      :style="{ gridColumn: index + 2, gridRow: 1 }"
    >
      <ContentHoverTip
        v-if="card"
        class="absolute inset-0 block"
        :title="card.name"
        :body-md="card.body_md"
      >
        <img
          v-if="secondaryImageSrc(card.slug)"
          :src="secondaryImageSrc(card.slug)"
          :alt="card.name"
          class="combat-esprit-hand-slot-image"
        />
        <span v-else class="combat-esprit-hand-slot-fallback">{{ card.name }}</span>
      </ContentHoverTip>
    </div>
    <span
      v-for="badge in badges"
      :key="`badge-${badge.number}-${badge.columnStart}`"
      class="combat-esprit-step-badge"
      :class="{
        'combat-esprit-step-badge--span': badge.span > 1,
        'combat-esprit-step-badge--current': currentStepNumber === badge.number,
      }"
      :style="{
        gridColumn: `${badge.columnStart} / span ${badge.span}`,
        gridRow: 1,
      }"
      aria-hidden="true"
    >
      <span>{{ badge.number }}</span>
    </span>
  </div>
</template>
