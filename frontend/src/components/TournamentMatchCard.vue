<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import TournamentMatchScoreboard from '@/components/TournamentMatchScoreboard.vue'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { formatMatchRecordedDate } from '@/lib/tournamentMatchDisplay'
import type { TournamentMatch } from '@/types/elo'

export interface TournamentMatchForm {
  p1: number
  p2: number
  s1: number
  s2: number
  list1?: number
  list2?: number
}

const props = withDefaults(
  defineProps<{
    match: TournamentMatch
    form: TournamentMatchForm
    canInteract: boolean
    isAdmin: boolean
    player1ArmyId?: number
    player2ArmyId?: number
    player1HasList2?: boolean
    player2HasList2?: boolean
    statusLabel: string
    phaseLabel?: string
    /** Affichage resserré (panneau latéral / mobile). */
    compact?: boolean
    /** Les deux joueurs ont leurs listes d'arbre (hors poules). */
    listsReady?: boolean
    listsReadyMessage?: string
  }>(),
  {
    compact: false,
    listsReady: true,
    listsReadyMessage: '',
    player1HasList2: false,
    player2HasList2: false,
  },
)

const emit = defineEmits<{
  submit: []
  confirm: []
  correct: [form: TournamentMatchForm]
  forfeit: [playerName: string]
  unplayed: []
}>()

const correcting = ref(false)

const hasBothPlayers = computed(
  () => Boolean(props.match.player1 && props.match.player2),
)

const isPoolMatch = computed(() => props.match.phase === 'pool')

const listLabel = computed(() =>
  isPoolMatch.value ? 'Liste d’inscription' : 'Liste d’arbre',
)

/** Affichage compact confirmé (scores sur une ligne). */
const showScoresView = computed(
  () => props.match.status === 'confirmed' && !correcting.value,
)

/** Saisie ou correction — jamais pour un match confirmé sauf mode correction admin. */
const showFormView = computed(() => {
  if (correcting.value) return true
  if (props.match.status === 'confirmed') return false
  return props.canInteract && hasBothPlayers.value
})

const scoreboardMode = computed(() => {
  if (showScoresView.value) return 'scores' as const
  if (showFormView.value) return 'form' as const
  return 'players' as const
})

const canSubmitScores = computed(
  () =>
    props.listsReady
    && (props.form.list1 === 1 || props.form.list1 === 2)
    && (props.form.list2 === 1 || props.form.list2 === 2)
    && (props.form.list1 !== 2 || props.player1HasList2)
    && (props.form.list2 !== 2 || props.player2HasList2),
)

watch(
  () => [props.player1HasList2, props.player2HasList2] as const,
  ([p1, p2]) => {
    if (!p1) props.form.list1 = 1
    if (!p2) props.form.list2 = 1
  },
  { immediate: true },
)

function startCorrection() {
  props.form.p1 = props.match.player1_objectives
  props.form.p2 = props.match.player2_objectives
  props.form.s1 = props.match.player1_survivors
  props.form.s2 = props.match.player2_survivors
  correcting.value = true
}

function cancelCorrection() {
  correcting.value = false
}

function saveCorrection() {
  emit('correct', {
    p1: Number(props.form.p1) || 0,
    p2: Number(props.form.p2) || 0,
    s1: Number(props.form.s1) || 0,
    s2: Number(props.form.s2) || 0,
    list1: props.form.list1,
    list2: props.form.list2,
  })
}

watch(
  () => [
    props.match.player1_objectives,
    props.match.player2_objectives,
    props.match.player1_survivors,
    props.match.player2_survivors,
  ],
  () => {
    correcting.value = false
  },
)

function matchPlayerLabel(slot: 'player1' | 'player2') {
  const name = props.match[slot]
  const displayName =
    slot === 'player1'
      ? props.match.player1_display_name
      : props.match.player2_display_name
  return displayName || name || '?'
}
</script>

<template>
  <div
    class="tournament-match-card"
    :class="{ 'tournament-match-card--compact': compact }"
  >
    <div class="tournament-match-meta">
      <span
        class="tournament-match-meta-phase"
        :class="{ 'tournament-match-meta-empty': !phaseLabel }"
      >
        {{ phaseLabel ?? '—' }}
      </span>
      <span
        class="tournament-match-meta-scenario"
        :class="{ 'tournament-match-meta-empty': !match.scenario_name }"
        :title="match.scenario_name ?? undefined"
      >
        {{ match.scenario_name ?? '—' }}
      </span>
      <span
        v-if="!compact"
        class="tournament-match-meta-date"
        :class="{ 'tournament-match-meta-empty': !match.played_at }"
      >
        {{ match.played_at ? (formatMatchRecordedDate(match.played_at) ?? '—') : '—' }}
      </span>
      <div class="tournament-match-meta-status-group">
        <span class="tournament-match-meta-status">
          {{ statusLabel }}
        </span>
        <Button
          v-if="isAdmin && match.status === 'confirmed' && !correcting"
          size="sm"
          variant="outline"
          class="tournament-match-correct-btn"
          @click="startCorrection"
        >
          Corriger
        </Button>
      </div>
    </div>

    <div class="tournament-match-layout">
      <TournamentMatchScoreboard
        :match="match"
        :mode="scoreboardMode"
        :form="form"
        :compact="compact"
        :player1-army-id="player1ArmyId"
        :player2-army-id="player2ArmyId"
      />

      <div v-if="showFormView" class="tournament-match-actions">
        <template v-if="match.status === 'confirmed' && correcting">
          <Button size="sm" @click="saveCorrection">
            Enregistrer
          </Button>
          <Button size="sm" variant="outline" @click="cancelCorrection">
            Annuler
          </Button>
        </template>
        <template v-else>
          <p
            v-if="!listsReady"
            class="w-full text-sm text-amber-600 dark:text-amber-400"
          >
            {{ listsReadyMessage || 'Listes d’arbre manquantes.' }}
          </p>
          <div
            v-else
            class="flex w-full flex-wrap items-end gap-3"
          >
            <div class="grid gap-1">
              <Label class="text-xs">{{ matchPlayerLabel('player1') }} — {{ listLabel }}</Label>
              <select
                v-if="player1HasList2"
                v-model.number="form.list1"
                class="h-8 rounded-md border border-input bg-transparent px-2 text-sm"
              >
                <option :value="undefined" disabled>Choisir…</option>
                <option :value="1">Liste 1</option>
                <option :value="2">Liste 2</option>
              </select>
              <span
                v-else
                class="flex h-8 items-center text-sm text-muted-foreground"
              >
                Liste 1
              </span>
            </div>
            <div class="grid gap-1">
              <Label class="text-xs">{{ matchPlayerLabel('player2') }} — {{ listLabel }}</Label>
              <select
                v-if="player2HasList2"
                v-model.number="form.list2"
                class="h-8 rounded-md border border-input bg-transparent px-2 text-sm"
              >
                <option :value="undefined" disabled>Choisir…</option>
                <option :value="1">Liste 1</option>
                <option :value="2">Liste 2</option>
              </select>
              <span
                v-else
                class="flex h-8 items-center text-sm text-muted-foreground"
              >
                Liste 1
              </span>
            </div>
          </div>
          <Button
            size="sm"
            :disabled="!canSubmitScores"
            @click="emit('submit')"
          >
            Soumettre
          </Button>
          <Button
            v-if="match.status === 'submitted'"
            size="sm"
            variant="outline"
            @click="emit('confirm')"
          >
            Confirmer
          </Button>
          <template v-if="isAdmin">
            <Button
              v-if="isPoolMatch"
              size="sm"
              variant="outline"
              @click="emit('unplayed')"
            >
              Match non joué
            </Button>
            <Button
              size="sm"
              variant="destructive"
              @click="emit('forfeit', match.player1!)"
            >
              FF {{ matchPlayerLabel('player1') }}
            </Button>
            <Button
              size="sm"
              variant="destructive"
              @click="emit('forfeit', match.player2!)"
            >
              FF {{ matchPlayerLabel('player2') }}
            </Button>
          </template>
        </template>
      </div>
    </div>
  </div>
</template>
