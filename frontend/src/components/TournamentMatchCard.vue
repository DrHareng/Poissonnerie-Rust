<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import TournamentMatchScoreboard from '@/components/TournamentMatchScoreboard.vue'
import { Button } from '@/components/ui/button'
import { formatMatchDate } from '@/lib/tournamentMatchDisplay'
import type { TournamentMatch } from '@/types/elo'

export interface TournamentMatchForm {
  p1: number
  p2: number
  s1: number
  s2: number
}

const props = defineProps<{
  match: TournamentMatch
  form: TournamentMatchForm
  canInteract: boolean
  isAdmin: boolean
  player1ArmyId?: number
  player2ArmyId?: number
  statusLabel: string
  phaseLabel?: string
}>()

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
  <div class="tournament-match-card">
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
        class="tournament-match-meta-date"
        :class="{ 'tournament-match-meta-empty': !match.played_at }"
      >
        {{ match.played_at ? formatMatchDate(match.played_at) : '—' }}
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
          <Button size="sm" @click="emit('submit')">Soumettre</Button>
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
