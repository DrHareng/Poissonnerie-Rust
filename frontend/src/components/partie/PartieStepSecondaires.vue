<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Dices } from '@lucide/vue'
import {
  COMBAT_ESPRIT_SLUG,
  activeDraftPlayer,
  createCombatEspritDraft,
  currentDraftStep,
  draftPlayerLabel,
  handForPlayer,
  isDraftComplete,
  mapDraftToPlayers,
  pickCard,
  runAutoSteps,
  type CombatEspritDraftState,
  type DraftPlayer,
} from '@/lib/combatEspritDraft'
import { shufflePick } from '@/lib/shufflePick'
import type { SecondaryObjective } from '@/types/elo'
import SecondaryCardGrid from '@/components/partie/SecondaryCardGrid.vue'
import { Button } from '@/components/ui/button'

const props = defineProps<{
  player1Name: string
  player2Name: string
  scenarioSlug?: string
  secondaries: SecondaryObjective[]
  loading?: boolean
  initialPlayer1?: string[]
  initialPlayer2?: string[]
}>()

const emit = defineEmits<{
  back: []
  next: [payload: { player1: string[]; player2: string[] }]
}>()

const isCombatEsprit = computed(
  () => props.scenarioSlug === COMBAT_ESPRIT_SLUG,
)

const drawnPlayer1 = ref<string[]>(props.initialPlayer1 ?? [])
const drawnPlayer2 = ref<string[]>(props.initialPlayer2 ?? [])

const draftState = ref<CombatEspritDraftState | null>(null)
const firstPicker = ref<DraftPlayer>('A')

const currentStep = computed(() =>
  draftState.value ? currentDraftStep(draftState.value) : null,
)

const draftComplete = computed(() =>
  draftState.value ? isDraftComplete(draftState.value) : false,
)

const activePlayer = computed(() =>
  draftState.value ? activeDraftPlayer(draftState.value) : null,
)

const deckCards = computed(() => draftState.value?.deck ?? [])

const player1Hand = computed(() => {
  if (!draftState.value) return []
  return handForPlayer(draftState.value, 1, firstPicker.value)
})

const player2Hand = computed(() => {
  if (!draftState.value) return []
  return handForPlayer(draftState.value, 2, firstPicker.value)
})

const canContinueStandard = computed(
  () => drawnPlayer1.value.length === 3 && drawnPlayer2.value.length === 3,
)

const canContinue = computed(() =>
  isCombatEsprit.value ? draftComplete.value : canContinueStandard.value,
)

const secondaryBySlug = computed(
  () => new Map(props.secondaries.map((item) => [item.slug, item])),
)

function resolveSlugs(slugs: string[]): SecondaryObjective[] {
  return slugs
    .map((slug) => secondaryBySlug.value.get(slug))
    .filter((item): item is SecondaryObjective => item != null)
}

function drawForPlayer(slot: 1 | 2) {
  if (props.secondaries.length < 3) return
  const picked = shufflePick(props.secondaries, 3).map((item) => item.slug)
  if (slot === 1) {
    drawnPlayer1.value = picked
  } else {
    drawnPlayer2.value = picked
  }
}

function drawBothPlayers() {
  drawForPlayer(1)
  drawForPlayer(2)
}

function startCombatEspritDraft() {
  firstPicker.value = Math.random() < 0.5 ? 'A' : 'B'
  draftState.value = createCombatEspritDraft(props.secondaries, firstPicker.value)
  runAutoSteps(draftState.value)
}

function onDraftPick(slug: string) {
  if (!draftState.value) return
  pickCard(draftState.value, slug)
  if (isDraftComplete(draftState.value)) {
    const mapped = mapDraftToPlayers(draftState.value, firstPicker.value)
    drawnPlayer1.value = mapped.player1
    drawnPlayer2.value = mapped.player2
  }
}

function activePlayerName(): string {
  if (!activePlayer.value) return ''
  return draftPlayerLabel(
    activePlayer.value,
    firstPicker.value,
    props.player1Name,
    props.player2Name,
  )
}

function submit() {
  if (!canContinue.value) return
  emit('next', {
    player1: drawnPlayer1.value,
    player2: drawnPlayer2.value,
  })
}

onMounted(() => {
  if (isCombatEsprit.value && drawnPlayer1.value.length === 0) {
    startCombatEspritDraft()
  }
})
</script>

<template>
  <div class="grid gap-6">
    <template v-if="isCombatEsprit">
      <p class="page-description">
        <strong>Le Combat de l'Esprit</strong> — un seul deck partagé. Tirage A/B :
        chaque joueur choisit puis reçoit des objectifs selon la séquence du scénario.
      </p>

      <div
        v-if="!draftComplete && currentStep"
        class="neon-panel space-y-3 rounded-lg border border-primary/25 p-4"
      >
        <p class="font-medium text-primary">{{ currentStep.label }}</p>
        <p v-if="currentStep.kind === 'pick'" class="text-sm text-muted-foreground">
          {{ activePlayerName() }}, choisissez un objectif dans le deck.
        </p>
        <p v-else class="text-sm text-muted-foreground">
          Tirage automatique en cours…
        </p>
        <SecondaryCardGrid
          v-if="currentStep.kind === 'pick'"
          :secondaries="deckCards"
          selectable
          @select="onDraftPick"
        />
      </div>

      <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <section class="player-match-panel">
          <p class="player-match-panel-title">{{ player1Name }}</p>
          <SecondaryCardGrid
            v-if="player1Hand.length > 0"
            :secondaries="player1Hand"
          />
          <p v-else class="text-sm text-muted-foreground">Aucun objectif pour l'instant.</p>
        </section>
        <section class="player-match-panel">
          <p class="player-match-panel-title">{{ player2Name }}</p>
          <SecondaryCardGrid
            v-if="player2Hand.length > 0"
            :secondaries="player2Hand"
          />
          <p v-else class="text-sm text-muted-foreground">Aucun objectif pour l'instant.</p>
        </section>
      </div>

      <Button
        v-if="draftComplete"
        type="button"
        variant="outline"
        class="w-fit"
        @click="startCombatEspritDraft"
      >
        <Dices class="size-4" />
        Relancer le draft
      </Button>
    </template>

    <template v-else>
      <p class="page-description">
        Avant le choix de la liste, chaque joueur pioche 3 objectifs secondaires.
      </p>

      <div class="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          :disabled="loading || secondaries.length < 3"
          @click="drawBothPlayers"
        >
          <Dices class="size-4" />
          Tirer pour les deux joueurs
        </Button>
      </div>

      <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <section class="player-match-panel">
          <div class="flex items-center justify-between gap-2">
            <p class="player-match-panel-title">{{ player1Name }}</p>
            <Button
              type="button"
              variant="ghost"
              size="xs"
              :disabled="loading || secondaries.length < 3"
              @click="drawForPlayer(1)"
            >
              <Dices class="size-3.5" />
              Tirer
            </Button>
          </div>
          <SecondaryCardGrid
            v-if="drawnPlayer1.length > 0"
            :secondaries="resolveSlugs(drawnPlayer1)"
          />
          <p v-else class="text-sm text-muted-foreground">
            Appuyez sur Tirer pour piocher 3 objectifs.
          </p>
        </section>

        <section class="player-match-panel">
          <div class="flex items-center justify-between gap-2">
            <p class="player-match-panel-title">{{ player2Name }}</p>
            <Button
              type="button"
              variant="ghost"
              size="xs"
              :disabled="loading || secondaries.length < 3"
              @click="drawForPlayer(2)"
            >
              <Dices class="size-3.5" />
              Tirer
            </Button>
          </div>
          <SecondaryCardGrid
            v-if="drawnPlayer2.length > 0"
            :secondaries="resolveSlugs(drawnPlayer2)"
          />
          <p v-else class="text-sm text-muted-foreground">
            Appuyez sur Tirer pour piocher 3 objectifs.
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
