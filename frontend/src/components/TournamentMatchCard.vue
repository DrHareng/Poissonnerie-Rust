<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import TournamentMatchScoreboard from '@/components/TournamentMatchScoreboard.vue'
import { Button } from '@/components/ui/button'
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
    /** Nom du joueur connecté (pour forfait soi-même). */
    currentPlayerName?: string | null
    player1ArmyId?: number
    player2ArmyId?: number
    player1HasList2?: boolean
    player2HasList2?: boolean
    statusLabel: string
    phaseLabel?: string
    compact?: boolean
    listsReady?: boolean
    listsReadyMessage?: string
  }>(),
  {
    compact: false,
    listsReady: true,
    listsReadyMessage: '',
    player1HasList2: false,
    player2HasList2: false,
    currentPlayerName: null,
  },
)

const emit = defineEmits<{
  start: []
  resume: []
  confirm: []
  correct: [form: TournamentMatchForm]
  forfeit: [playerName: string]
  cancelForfeit: []
  unplayed: []
}>()

const correcting = ref(false)

const hasBothPlayers = computed(
  () => Boolean(props.match.player1 && props.match.player2),
)

const isPoolMatch = computed(() => props.match.phase === 'pool')

const showScoresView = computed(
  () =>
    (props.match.status === 'confirmed' || props.match.status === 'submitted')
    && !correcting.value,
)

const scoreboardMode = computed(() => {
  if (correcting.value) return 'form' as const
  if (showScoresView.value) return 'scores' as const
  return 'players' as const
})

const canStart = computed(
  () =>
    props.canInteract
    && hasBothPlayers.value
    && props.match.status === 'scheduled'
    && !props.match.is_forfeit
    && !props.match.is_unplayed
    && props.listsReady
    && !props.match.elo_match_id,
)

const canResume = computed(
  () =>
    props.canInteract
    && Boolean(props.match.elo_match_id)
    && props.match.status === 'scheduled'
    && !props.match.is_forfeit
    && !props.match.is_unplayed,
)

const canConfirm = computed(
  () => props.canInteract && props.match.status === 'submitted',
)

const selfForfeitName = computed(() => {
  const me = props.currentPlayerName?.toLowerCase()
  if (!me) return null
  if (props.match.player1?.toLowerCase() === me) return props.match.player1
  if (props.match.player2?.toLowerCase() === me) return props.match.player2
  return null
})

watch(
  () => [
    props.match.player1_objectives,
    props.match.player2_objectives,
    props.match.player1_survivors,
    props.match.player2_survivors,
    props.match.status,
  ],
  () => {
    correcting.value = false
  },
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
  })
}

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
          v-if="isAdmin && match.status === 'confirmed' && !match.is_forfeit && !correcting"
          size="sm"
          variant="outline"
          class="tournament-match-correct-btn"
          @click="startCorrection"
        >
          Corriger
        </Button>
        <Button
          v-if="isAdmin && match.is_forfeit"
          size="sm"
          variant="outline"
          class="tournament-match-correct-btn"
          @click="emit('cancelForfeit')"
        >
          Annuler forfait
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

      <div
        v-if="canInteract && hasBothPlayers"
        class="tournament-match-actions"
      >
        <template v-if="correcting">
          <Button size="sm" @click="saveCorrection">
            Enregistrer
          </Button>
          <Button size="sm" variant="outline" @click="cancelCorrection">
            Annuler
          </Button>
        </template>
        <template v-else>
          <p
            v-if="!listsReady && match.status === 'scheduled'"
            class="w-full text-sm text-amber-600 dark:text-amber-400"
          >
            {{ listsReadyMessage || 'Listes d’arbre manquantes.' }}
          </p>

          <Button
            v-if="canStart"
            size="sm"
            @click="emit('start')"
          >
            Démarrer la partie
          </Button>
          <Button
            v-if="canResume"
            size="sm"
            @click="emit('resume')"
          >
            Reprendre la partie
          </Button>
          <Button
            v-if="canConfirm"
            size="sm"
            variant="outline"
            @click="emit('confirm')"
          >
            {{ match.is_forfeit ? 'Confirmer le forfait' : 'Confirmer' }}
          </Button>

          <template v-if="match.status === 'scheduled' && !match.is_unplayed">
            <Button
              v-if="selfForfeitName"
              size="sm"
              variant="destructive"
              @click="emit('forfeit', selfForfeitName)"
            >
              Je déclare forfait
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
        </template>
      </div>
    </div>
  </div>
</template>
