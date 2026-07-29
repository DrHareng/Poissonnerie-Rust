<script setup lang="ts">
import { ref, watch } from 'vue'
import ArmyLogo from '@/components/ArmyLogo.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import TournamentMatchScoreboard from '@/components/TournamentMatchScoreboard.vue'
import { Button } from '@/components/ui/button'
import { matchPlayerScores } from '@/lib/tournamentMatchDisplay'
import type { TournamentMatchForm } from '@/components/TournamentMatchCard.vue'
import type { TournamentMatch } from '@/types/elo'

const props = withDefaults(
  defineProps<{
    matches: TournamentMatch[]
    isAdmin: boolean
    getForm: (match: TournamentMatch) => TournamentMatchForm
    canInteract: (match: TournamentMatch) => boolean
    playerArmyId: (match: TournamentMatch, slot: 'player1' | 'player2') => number | undefined
    statusLabel: (match: TournamentMatch) => string
    allowUnplayed?: boolean
  }>(),
  { allowUnplayed: true },
)

const emit = defineEmits<{
  submit: [match: TournamentMatch]
  confirm: [match: TournamentMatch]
  correct: [match: TournamentMatch, form: TournamentMatchForm]
  forfeit: [match: TournamentMatch, playerName: string]
  unplayed: [match: TournamentMatch]
}>()

const editingMatchId = ref<number | null>(null)
const correctingMatchId = ref<number | null>(null)

function hasBothPlayers(match: TournamentMatch) {
  return Boolean(match.player1 && match.player2)
}

function canSaisir(match: TournamentMatch) {
  return (
    props.canInteract(match)
    && match.status !== 'confirmed'
    && hasBothPlayers(match)
    && editingMatchId.value !== match.id
  )
}

function canCorriger(match: TournamentMatch) {
  return (
    props.isAdmin
    && match.status === 'confirmed'
    && editingMatchId.value !== match.id
  )
}

function canConfirm(match: TournamentMatch) {
  return props.isAdmin && match.status === 'submitted'
}

function isEditing(match: TournamentMatch) {
  return editingMatchId.value === match.id
}

function startSaisir(match: TournamentMatch) {
  correctingMatchId.value = null
  editingMatchId.value = match.id
}

function startCorrection(match: TournamentMatch) {
  const form = props.getForm(match)
  form.p1 = match.player1_objectives
  form.p2 = match.player2_objectives
  form.s1 = match.player1_survivors
  form.s2 = match.player2_survivors
  correctingMatchId.value = match.id
  editingMatchId.value = match.id
}

function cancelEdit() {
  editingMatchId.value = null
  correctingMatchId.value = null
}

function saveCorrection(match: TournamentMatch) {
  const form = props.getForm(match)
  emit('correct', match, {
    p1: Number(form.p1) || 0,
    p2: Number(form.p2) || 0,
    s1: Number(form.s1) || 0,
    s2: Number(form.s2) || 0,
  })
}

function matchPlayerLabel(match: TournamentMatch, slot: 'player1' | 'player2') {
  const displayName =
    slot === 'player1' ? match.player1_display_name : match.player2_display_name
  const name = match[slot]
  return displayName || name || '?'
}

function hasScores(match: TournamentMatch) {
  return (
    match.status === 'confirmed'
    || match.status === 'submitted'
    || match.is_forfeit
    || match.is_unplayed
  )
}

function formatScoreLine(match: TournamentMatch, slot: 'player1' | 'player2') {
  if (!hasScores(match)) return '—'
  const { pt, po, ps } = matchPlayerScores(match, slot)
  if (match.status === 'confirmed') {
    return `${pt} PT · ${po} PO · ${ps} PS`
  }
  return `${po} PO · ${ps} PS`
}

watch(
  () => props.matches.map((m) => m.id).join(','),
  () => {
    editingMatchId.value = null
    correctingMatchId.value = null
  },
)

watch(
  () =>
    props.matches.flatMap((m) => [
      m.player1_objectives,
      m.player2_objectives,
      m.player1_survivors,
      m.player2_survivors,
      m.status,
    ]),
  () => {
    editingMatchId.value = null
    correctingMatchId.value = null
  },
)
</script>

<template>
  <table class="pool-matches-table">
    <thead>
      <tr>
        <th class="pool-col-player">Joueur 1</th>
        <th class="pool-col-score">Score</th>
        <th class="pool-col-player">Joueur 2</th>
        <th class="pool-col-admin" />
        <th class="pool-col-status">Statut</th>
      </tr>
    </thead>
    <tbody>
      <template v-for="match in matches" :key="match.id">
        <tr>
          <td class="pool-col-player">
            <span class="flex min-w-0 items-center gap-2">
              <PlayerLink
                v-if="match.player1"
                :name="match.player1"
                :display-name="match.player1_display_name"
              />
              <span v-else class="text-muted-foreground">?</span>
              <ArmyLogo
                v-if="playerArmyId(match, 'player1')"
                :army-id="playerArmyId(match, 'player1')!"
                class="shrink-0"
              />
            </span>
          </td>
          <td class="pool-col-score">
            <span class="pool-match-score-line tabular-nums">
              <span>{{ formatScoreLine(match, 'player1') }}</span>
              <span class="pool-match-score-sep" aria-hidden="true">—</span>
              <span>{{ formatScoreLine(match, 'player2') }}</span>
            </span>
          </td>
          <td class="pool-col-player">
            <span class="flex min-w-0 items-center gap-2">
              <PlayerLink
                v-if="match.player2"
                :name="match.player2"
                :display-name="match.player2_display_name"
              />
              <span v-else class="text-muted-foreground">?</span>
              <ArmyLogo
                v-if="playerArmyId(match, 'player2')"
                :army-id="playerArmyId(match, 'player2')!"
                class="shrink-0"
              />
            </span>
          </td>
          <td class="pool-col-admin">
            <div class="pool-match-admin-actions">
              <Button
                v-if="canSaisir(match)"
                size="sm"
                variant="outline"
                @click="startSaisir(match)"
              >
                Saisir
              </Button>
              <Button
                v-if="canCorriger(match)"
                size="sm"
                variant="outline"
                @click="startCorrection(match)"
              >
                Corriger
              </Button>
              <Button
                v-if="canConfirm(match) && !isEditing(match)"
                size="sm"
                @click="emit('confirm', match)"
              >
                Confirmer
              </Button>
            </div>
          </td>
          <td class="pool-col-status">
            {{ statusLabel(match) }}
          </td>
        </tr>
        <tr v-if="isEditing(match)" class="pool-match-edit-row">
          <td colspan="5">
            <div class="pool-match-edit-panel">
              <TournamentMatchScoreboard
                :match="match"
                mode="form"
                :form="getForm(match)"
                :player1-army-id="playerArmyId(match, 'player1')"
                :player2-army-id="playerArmyId(match, 'player2')"
              />
              <div class="pool-match-edit-actions">
                <template v-if="correctingMatchId === match.id">
                  <Button size="sm" @click="saveCorrection(match)">
                    Enregistrer
                  </Button>
                  <Button size="sm" variant="outline" @click="cancelEdit">
                    Annuler
                  </Button>
                </template>
                <template v-else>
                  <Button size="sm" @click="emit('submit', match)">
                    Soumettre
                  </Button>
                  <Button
                    v-if="match.status === 'submitted'"
                    size="sm"
                    variant="outline"
                    @click="emit('confirm', match)"
                  >
                    Confirmer
                  </Button>
                  <template v-if="isAdmin">
                    <Button
                      v-if="allowUnplayed"
                      size="sm"
                      variant="outline"
                      @click="emit('unplayed', match)"
                    >
                      Match non joué
                    </Button>
                    <Button
                      size="sm"
                      variant="destructive"
                      @click="emit('forfeit', match, match.player1!)"
                    >
                      FF {{ matchPlayerLabel(match, 'player1') }}
                    </Button>
                    <Button
                      size="sm"
                      variant="destructive"
                      @click="emit('forfeit', match, match.player2!)"
                    >
                      FF {{ matchPlayerLabel(match, 'player2') }}
                    </Button>
                  </template>
                  <Button size="sm" variant="outline" @click="cancelEdit">
                    Annuler
                  </Button>
                </template>
              </div>
            </div>
          </td>
        </tr>
      </template>
    </tbody>
  </table>
</template>
