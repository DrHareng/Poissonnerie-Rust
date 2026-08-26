<script setup lang="ts">
import { computed, ref, watch } from 'vue'
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
    currentPlayerName?: string | null
    getForm: (match: TournamentMatch) => TournamentMatchForm
    canInteract: (match: TournamentMatch) => boolean
    playerArmyId: (match: TournamentMatch, slot: 'player1' | 'player2') => number | undefined
    playerHasList2?: (match: TournamentMatch, slot: 'player1' | 'player2') => boolean
    statusLabel: (match: TournamentMatch) => string
    allowUnplayed?: boolean
    listsReady?: (match: TournamentMatch) => boolean
    listsReadyMessage?: (match: TournamentMatch) => string
  }>(),
  {
    allowUnplayed: true,
    currentPlayerName: null,
  },
)

const emit = defineEmits<{
  start: [match: TournamentMatch]
  resume: [match: TournamentMatch]
  confirm: [match: TournamentMatch]
  correct: [match: TournamentMatch, form: TournamentMatchForm]
  forfeit: [match: TournamentMatch, playerName: string]
  cancelForfeit: [match: TournamentMatch]
  unplayed: [match: TournamentMatch]
}>()

const correctingMatchId = ref<number | null>(null)

function isCorrecting(match: TournamentMatch) {
  return correctingMatchId.value === match.id
}

function canStart(match: TournamentMatch) {
  return (
    props.canInteract(match)
    && Boolean(match.player1 && match.player2)
    && match.status === 'scheduled'
    && !match.is_forfeit
    && !match.is_unplayed
    && !match.elo_match_id
    && (props.listsReady?.(match) ?? true)
  )
}

function canResume(match: TournamentMatch) {
  return (
    props.canInteract(match)
    && Boolean(match.elo_match_id)
    && match.status === 'scheduled'
    && !match.is_forfeit
    && !match.is_unplayed
  )
}

function canConfirm(match: TournamentMatch) {
  return props.canInteract(match) && match.status === 'submitted'
}

function selfForfeitName(match: TournamentMatch) {
  const me = props.currentPlayerName?.toLowerCase()
  if (!me) return null
  if (match.player1?.toLowerCase() === me) return match.player1
  if (match.player2?.toLowerCase() === me) return match.player2
  return null
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

function startCorrection(match: TournamentMatch) {
  const form = props.getForm(match)
  form.p1 = match.player1_objectives
  form.p2 = match.player2_objectives
  form.s1 = match.player1_survivors
  form.s2 = match.player2_survivors
  correctingMatchId.value = match.id
}

function saveCorrection(match: TournamentMatch) {
  const form = props.getForm(match)
  emit('correct', match, {
    p1: Number(form.p1) || 0,
    p2: Number(form.p2) || 0,
    s1: Number(form.s1) || 0,
    s2: Number(form.s2) || 0,
  })
  correctingMatchId.value = null
}

watch(
  () => props.matches.map((m) => `${m.id}:${m.status}:${m.player1_objectives}`).join(','),
  () => {
    correctingMatchId.value = null
  },
)

const showActions = computed(() => true)
void showActions
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
                v-if="canStart(match)"
                size="sm"
                @click="emit('start', match)"
              >
                Démarrer
              </Button>
              <Button
                v-if="canResume(match)"
                size="sm"
                @click="emit('resume', match)"
              >
                Reprendre
              </Button>
              <Button
                v-if="canConfirm(match)"
                size="sm"
                variant="outline"
                @click="emit('confirm', match)"
              >
                {{ match.is_forfeit ? 'Confirmer FF' : 'Confirmer' }}
              </Button>
              <Button
                v-if="isAdmin && match.status === 'confirmed' && !match.is_forfeit && !isCorrecting(match)"
                size="sm"
                variant="outline"
                @click="startCorrection(match)"
              >
                Corriger
              </Button>
              <Button
                v-if="isAdmin && match.is_forfeit"
                size="sm"
                variant="outline"
                @click="emit('cancelForfeit', match)"
              >
                Annuler FF
              </Button>
            </div>
          </td>
          <td class="pool-col-status">
            {{ statusLabel(match) }}
          </td>
        </tr>
        <tr
          v-if="isCorrecting(match) || (canInteract(match) && match.status === 'scheduled' && !match.is_unplayed)"
          class="pool-match-edit-row"
        >
          <td colspan="5">
            <div class="pool-match-edit-panel">
              <template v-if="isCorrecting(match)">
                <TournamentMatchScoreboard
                  :match="match"
                  mode="form"
                  :form="getForm(match)"
                  :player1-army-id="playerArmyId(match, 'player1')"
                  :player2-army-id="playerArmyId(match, 'player2')"
                />
                <div class="pool-match-edit-actions">
                  <Button size="sm" @click="saveCorrection(match)">
                    Enregistrer
                  </Button>
                  <Button size="sm" variant="outline" @click="correctingMatchId = null">
                    Annuler
                  </Button>
                </div>
              </template>
              <template v-else>
                <p
                  v-if="listsReady && !listsReady(match)"
                  class="text-sm text-amber-600 dark:text-amber-400"
                >
                  {{ listsReadyMessage?.(match) || 'Listes d’arbre manquantes.' }}
                </p>
                <div class="pool-match-edit-actions">
                  <Button
                    v-if="selfForfeitName(match)"
                    size="sm"
                    variant="destructive"
                    @click="emit('forfeit', match, selfForfeitName(match)!)"
                  >
                    Je déclare forfait
                  </Button>
                  <template v-if="isAdmin">
                    <Button
                      v-if="allowUnplayed && match.phase === 'pool'"
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
                </div>
              </template>
            </div>
          </td>
        </tr>
      </template>
    </tbody>
  </table>
</template>
