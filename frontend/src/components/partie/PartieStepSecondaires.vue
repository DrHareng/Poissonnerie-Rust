<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Undo2 } from '@lucide/vue'
import {
  COMBAT_ESPRIT_DRAFT_STEPS,
  COMBAT_ESPRIT_SLUG,
  activeDraftPlayer,
  bannedForDisplay,
  canUndoDraft,
  chooseCard,
  createCombatEspritDraft,
  currentDraftStep,
  draftPlayerIsSlot1,
  draftPlayerLabel,
  draftStepBadges,
  handForPlayer,
  isDraftComplete,
  mapDraftToPlayers,
  undoDraft,
  type CombatEspritDraftState,
  type DraftPlayer,
} from '@/lib/combatEspritDraft'
import { secondaryImageSrc } from '@/lib/secondaryImages'
import type { SecondaryObjective } from '@/types/elo'
import CombatEspritPoolStrip from '@/components/partie/CombatEspritPoolStrip.vue'
import SecondaryCardGrid from '@/components/partie/SecondaryCardGrid.vue'
import ContentHoverTip from '@/components/ContentHoverTip.vue'
import { Button } from '@/components/ui/button'

const props = defineProps<{
  player1Name: string
  player2Name: string
  scenarioSlug?: string
  secondaries: SecondaryObjective[]
  loading?: boolean
  initialPlayer1?: string[]
  initialPlayer2?: string[]
  initialChosenPlayer1?: string | null
  initialChosenPlayer2?: string | null
  initialPool?: string[] | null
}>()

const emit = defineEmits<{
  back: []
  next: [
    payload: {
      player1: string[]
      player2: string[]
      chosenPlayer1: string | null
      chosenPlayer2: string | null
      pool: string[] | null
    },
  ]
}>()

const isCombatEsprit = computed(
  () => props.scenarioSlug === COMBAT_ESPRIT_SLUG,
)

const drawnPlayer1 = ref<string[]>([])
const drawnPlayer2 = ref<string[]>([])
const chosenPlayer1 = ref<string | null>(props.initialChosenPlayer1 ?? null)
const chosenPlayer2 = ref<string | null>(props.initialChosenPlayer2 ?? null)
const poolSlugs = ref<string[]>([])
const draftState = ref<CombatEspritDraftState | null>(null)
const firstPicker = ref<DraftPlayer>('A')
const draftRestored = ref(false)
const draftTick = ref(0)

const currentStep = computed(() => {
  draftTick.value
  return draftState.value ? currentDraftStep(draftState.value) : null
})

const draftComplete = computed(() => {
  draftTick.value
  if (draftRestored.value) return true
  return draftState.value ? isDraftComplete(draftState.value) : false
})

const activePlayer = computed(() => {
  draftTick.value
  return draftState.value ? activeDraftPlayer(draftState.value) : null
})

const starterName = computed(() =>
  draftPlayerLabel('A', firstPicker.value, props.player1Name, props.player2Name),
)

const secondaryBySlug = computed(
  () => new Map(props.secondaries.map((item) => [item.slug, item])),
)

const stripItems = computed(() => {
  draftTick.value
  if (draftState.value) {
    return draftState.value.slots.map((slot) => ({
      slug: slot.secondary.slug,
      name: slot.secondary.name,
      bodyMd: slot.secondary.body_md,
      status: slot.status,
      owner: slot.by
        ? draftPlayerIsSlot1(slot.by, firstPicker.value)
          ? ('player1' as const)
          : ('player2' as const)
        : null,
    }))
  }

  if (poolSlugs.value.length === 0) return []

  const p1 = new Set(drawnPlayer1.value)
  const p2 = new Set(drawnPlayer2.value)
  const banned = new Set(
    [chosenPlayer1.value, chosenPlayer2.value].filter(
      (slug): slug is string => Boolean(slug),
    ),
  )

  return poolSlugs.value.map((slug) => {
    const secondary = secondaryBySlug.value.get(slug)
    if (banned.has(slug)) {
      return {
        slug,
        name: secondary?.name,
        bodyMd: secondary?.body_md,
        status: 'banned' as const,
        owner: null,
      }
    }
    if (p1.has(slug) || p2.has(slug)) {
      return {
        slug,
        name: secondary?.name,
        bodyMd: secondary?.body_md,
        status: 'taken' as const,
        owner: p1.has(slug) ? ('player1' as const) : ('player2' as const),
      }
    }
    return {
      slug,
      name: secondary?.name,
      bodyMd: secondary?.body_md,
      status: 'available' as const,
      owner: null,
    }
  })
})

const player1Hand = computed(() => {
  draftTick.value
  if (!draftState.value) return resolveSlugs(drawnPlayer1.value)
  return handForPlayer(draftState.value, 1, firstPicker.value)
})

const player2Hand = computed(() => {
  draftTick.value
  if (!draftState.value) return resolveSlugs(drawnPlayer2.value)
  return handForPlayer(draftState.value, 2, firstPicker.value)
})

/** Toujours 3 emplacements, remplis dans l’ordre de prise. */
function handSlots(hand: SecondaryObjective[]) {
  return [0, 1, 2].map((index) => hand[index] ?? null)
}

const player1Slots = computed(() => handSlots(player1Hand.value))
const player2Slots = computed(() => handSlots(player2Hand.value))

const bannedSplit = computed(() => {
  draftTick.value
  if (!draftState.value) {
    return {
      left: resolveSlugs(chosenPlayer1.value ? [chosenPlayer1.value] : []),
      right: resolveSlugs(chosenPlayer2.value ? [chosenPlayer2.value] : []),
    }
  }
  return bannedForDisplay(draftState.value, firstPicker.value)
})

const player1Ban = computed(() => bannedSplit.value.left[0] ?? null)
const player2Ban = computed(() => bannedSplit.value.right[0] ?? null)

const canUndo = computed(() => {
  draftTick.value
  return draftState.value ? canUndoDraft(draftState.value) : false
})

const activeSlot = computed<'player1' | 'player2' | null>(() => {
  if (!activePlayer.value) return null
  return draftPlayerIsSlot1(activePlayer.value, firstPicker.value)
    ? 'player1'
    : 'player2'
})

const player1StepBadges = computed(() => draftStepBadges(firstPicker.value))

const player2StepBadges = computed(() =>
  draftStepBadges(firstPicker.value === 'A' ? 'B' : 'A'),
)

const currentStepNumber = computed(() => {
  draftTick.value
  if (!draftState.value || draftRestored.value) return null
  if (draftState.value.stepIndex >= COMBAT_ESPRIT_DRAFT_STEPS.length) {
    return null
  }
  return draftState.value.stepIndex + 1
})

const canContinueStandard = computed(
  () =>
    drawnPlayer1.value.length === 3 &&
    drawnPlayer2.value.length === 3 &&
    Boolean(chosenPlayer1.value) &&
    Boolean(chosenPlayer2.value) &&
    drawnPlayer1.value.includes(chosenPlayer1.value!) &&
    drawnPlayer2.value.includes(chosenPlayer2.value!),
)

const canContinue = computed(() =>
  isCombatEsprit.value ? draftComplete.value : canContinueStandard.value,
)

function bumpDraft() {
  draftTick.value += 1
}

function resolveSlugs(slugs: string[]): SecondaryObjective[] {
  return slugs
    .map((slug) => secondaryBySlug.value.get(slug))
    .filter((item): item is SecondaryObjective => item != null)
}

function syncDrawnFromDraft() {
  if (!draftState.value || !isDraftComplete(draftState.value)) return
  const mapped = mapDraftToPlayers(draftState.value, firstPicker.value)
  drawnPlayer1.value = mapped.player1
  drawnPlayer2.value = mapped.player2
  chosenPlayer1.value = mapped.bannedPlayer1
  chosenPlayer2.value = mapped.bannedPlayer2
  poolSlugs.value = mapped.pool
}

function startCombatEspritDraft() {
  draftRestored.value = false
  firstPicker.value = Math.random() < 0.5 ? 'A' : 'B'
  draftState.value = createCombatEspritDraft(props.secondaries, firstPicker.value)
  bumpDraft()
}

function onDraftChoose(slug: string) {
  if (!draftState.value || draftComplete.value) return
  if (!chooseCard(draftState.value, slug)) return
  bumpDraft()
  syncDrawnFromDraft()
}

function onUndo() {
  if (!draftState.value) return
  if (!undoDraft(draftState.value)) return
  bumpDraft()
}

function ensureDrawn() {
  if (isCombatEsprit.value) {
    if (draftState.value) return

    if (
      (props.initialPlayer1?.length ?? 0) > 0 &&
      (props.initialPlayer2?.length ?? 0) > 0
    ) {
      drawnPlayer1.value = [...(props.initialPlayer1 ?? [])]
      drawnPlayer2.value = [...(props.initialPlayer2 ?? [])]
      chosenPlayer1.value = props.initialChosenPlayer1 ?? null
      chosenPlayer2.value = props.initialChosenPlayer2 ?? null
      poolSlugs.value = [...(props.initialPool ?? [])]
      draftRestored.value = true
      return
    }

    if (props.secondaries.length > 0) {
      startCombatEspritDraft()
    }
    return
  }

  draftRestored.value = false
  poolSlugs.value = []

  if (
    (props.initialPlayer1?.length ?? 0) >= 3 &&
    (props.initialPlayer2?.length ?? 0) >= 3
  ) {
    drawnPlayer1.value = [...props.initialPlayer1!]
    drawnPlayer2.value = [...props.initialPlayer2!]
    chosenPlayer1.value = props.initialChosenPlayer1 ?? null
    chosenPlayer2.value = props.initialChosenPlayer2 ?? null
  }
}

function submit() {
  if (!canContinue.value) return
  emit('next', {
    player1: drawnPlayer1.value,
    player2: drawnPlayer2.value,
    chosenPlayer1: chosenPlayer1.value,
    chosenPlayer2: chosenPlayer2.value,
    pool: isCombatEsprit.value ? poolSlugs.value : null,
  })
}

onMounted(ensureDrawn)

watch(
  () => [
    props.secondaries,
    props.initialPlayer1,
    props.initialPlayer2,
    props.initialChosenPlayer1,
    props.initialChosenPlayer2,
    props.initialPool,
    props.scenarioSlug,
  ],
  () => {
    ensureDrawn()
  },
)
</script>

<template>
  <div class="grid gap-6">
    <template v-if="isCombatEsprit">
      <p v-if="draftRestored" class="page-description">
        Le combat de l'esprit — tirage déjà effectué.
      </p>
      <p v-else class="page-description">
        Le combat de l'esprit,
        <span class="font-medium text-primary">{{ starterName }}</span>
        a été tiré au sort pour commencer.
      </p>

      <!-- Rangée 1 : joueurs + timeline + ban + 3 picks -->
      <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <section
          class="player-match-panel"
          :class="{ 'combat-esprit-player--active': activeSlot === 'player1' }"
        >
          <div class="combat-esprit-player-header">
            <p class="player-match-panel-title">{{ player1Name }}</p>
          </div>
          <div class="combat-esprit-hand-slots">
            <div
              class="combat-esprit-hand-slot combat-esprit-hand-slot--ban"
              :class="{ 'combat-esprit-hand-slot--filled': !!player1Ban }"
              style="grid-column: 1; grid-row: 1"
            >
              <ContentHoverTip
                v-if="player1Ban"
                class="absolute inset-0 block"
                :title="player1Ban.name"
                :body-md="player1Ban.body_md"
              >
                <img
                  v-if="secondaryImageSrc(player1Ban.slug)"
                  :src="secondaryImageSrc(player1Ban.slug)"
                  :alt="player1Ban.name"
                  class="combat-esprit-hand-slot-image combat-esprit-hand-slot-image--banned"
                />
                <span v-else class="combat-esprit-hand-slot-fallback">{{
                  player1Ban.name
                }}</span>
              </ContentHoverTip>
              <span class="combat-esprit-ban-card-mark" aria-hidden="true">
                <svg viewBox="0 0 100 100" preserveAspectRatio="none">
                  <line x1="0" y1="0" x2="100" y2="100" />
                  <line x1="100" y1="0" x2="0" y2="100" />
                </svg>
              </span>
            </div>
            <div
              v-for="(card, index) in player1Slots"
              :key="`p1-slot-${index}`"
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
                <span v-else class="combat-esprit-hand-slot-fallback">{{
                  card.name
                }}</span>
              </ContentHoverTip>
            </div>
            <span
              v-for="badge in player1StepBadges"
              :key="`p1-badge-${badge.number}-${badge.columnStart}`"
              class="combat-esprit-step-badge"
              :class="{
                'combat-esprit-step-badge--span': badge.span > 1,
                'combat-esprit-step-badge--current':
                  currentStepNumber === badge.number,
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
        </section>
        <section
          class="player-match-panel"
          :class="{ 'combat-esprit-player--active': activeSlot === 'player2' }"
        >
          <div class="combat-esprit-player-header">
            <p class="player-match-panel-title">{{ player2Name }}</p>
          </div>
          <div class="combat-esprit-hand-slots">
            <div
              class="combat-esprit-hand-slot combat-esprit-hand-slot--ban"
              :class="{ 'combat-esprit-hand-slot--filled': !!player2Ban }"
              style="grid-column: 1; grid-row: 1"
            >
              <ContentHoverTip
                v-if="player2Ban"
                class="absolute inset-0 block"
                :title="player2Ban.name"
                :body-md="player2Ban.body_md"
              >
                <img
                  v-if="secondaryImageSrc(player2Ban.slug)"
                  :src="secondaryImageSrc(player2Ban.slug)"
                  :alt="player2Ban.name"
                  class="combat-esprit-hand-slot-image combat-esprit-hand-slot-image--banned"
                />
                <span v-else class="combat-esprit-hand-slot-fallback">{{
                  player2Ban.name
                }}</span>
              </ContentHoverTip>
              <span class="combat-esprit-ban-card-mark" aria-hidden="true">
                <svg viewBox="0 0 100 100" preserveAspectRatio="none">
                  <line x1="0" y1="0" x2="100" y2="100" />
                  <line x1="100" y1="0" x2="0" y2="100" />
                </svg>
              </span>
            </div>
            <div
              v-for="(card, index) in player2Slots"
              :key="`p2-slot-${index}`"
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
                <span v-else class="combat-esprit-hand-slot-fallback">{{
                  card.name
                }}</span>
              </ContentHoverTip>
            </div>
            <span
              v-for="badge in player2StepBadges"
              :key="`p2-badge-${badge.number}-${badge.columnStart}`"
              class="combat-esprit-step-badge"
              :class="{
                'combat-esprit-step-badge--span': badge.span > 1,
                'combat-esprit-step-badge--current':
                  currentStepNumber === badge.number,
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
        </section>
      </div>

      <div v-if="draftState && !draftRestored" class="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          :disabled="!canUndo"
          @click="onUndo"
        >
          <Undo2 class="size-4" />
          Revenir en arrière
        </Button>
      </div>

      <!-- Rangée 2 : 8 emplacements fixes -->
      <section class="player-match-panel space-y-3">
        <p class="player-match-panel-title">Objectifs</p>
        <CombatEspritPoolStrip
          v-if="stripItems.length > 0"
          :items="stripItems"
          hide-taken
          :selectable="!draftComplete && !!currentStep"
          @select="onDraftChoose"
        />
        <p v-else class="text-sm text-muted-foreground">
          Tirage du deck en cours…
        </p>
      </section>
    </template>

    <template v-else>
      <p class="page-description">
        Chaque joueur pioche 3 objectifs et en choisit 1 via le cercle. Cliquez sur une
        carte pour l'agrandir.
      </p>

      <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <section class="player-match-panel">
          <p class="player-match-panel-title">{{ player1Name }}</p>
          <SecondaryCardGrid
            v-if="drawnPlayer1.length > 0"
            :secondaries="resolveSlugs(drawnPlayer1)"
            viewable
            choosable
            choice-name="secondary-player1"
            :selected-slug="chosenPlayer1 ?? undefined"
            @choose="chosenPlayer1 = $event"
          />
          <p v-else class="text-sm text-muted-foreground">
            Tirage en cours…
          </p>
        </section>

        <section class="player-match-panel">
          <p class="player-match-panel-title">{{ player2Name }}</p>
          <SecondaryCardGrid
            v-if="drawnPlayer2.length > 0"
            :secondaries="resolveSlugs(drawnPlayer2)"
            viewable
            choosable
            choice-name="secondary-player2"
            :selected-slug="chosenPlayer2 ?? undefined"
            @choose="chosenPlayer2 = $event"
          />
          <p v-else class="text-sm text-muted-foreground">
            Tirage en cours…
          </p>
        </section>
      </div>
    </template>

    <div class="flex flex-col gap-2 sm:flex-row sm:justify-between">
      <Button type="button" variant="outline" @click="emit('back')">
        Précédent
      </Button>
      <Button type="button" :disabled="!canContinue" @click="submit">
        Valider
      </Button>
    </div>
  </div>
</template>
