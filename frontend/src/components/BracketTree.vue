<script setup lang="ts">
import { computed } from 'vue'
import ArmyLogo from '@/components/ArmyLogo.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import { phaseLabels } from '@/lib/tournamentPhase'
import {
  bracketMatchWinner,
  matchHasResult,
} from '@/lib/tournamentMatchDisplay'
import type { TournamentMatch, TournamentPhase } from '@/types/elo'

const props = withDefaults(
  defineProps<{
    matches: TournamentMatch[]
    /** Pastilles plus compactes pour la liste des tournois. */
    compact?: boolean
  }>(),
  { compact: false },
)

const phaseOrder: TournamentPhase[] = [
  'round_of_16',
  'quarter',
  'semi',
  'final',
]

type ConnectorMode = 'pair' | 'direct'

interface BracketRound {
  phase: TournamentPhase
  label: string
  matches: TournamentMatch[]
}

interface BracketSegment {
  round: BracketRound
  connector: { mode: ConnectorMode; count: number } | null
}

const rounds = computed((): BracketRound[] =>
  phaseOrder
    .map((phase) => ({
      phase,
      label: phaseLabels[phase],
      matches: props.matches
        .filter((match) => match.phase === phase)
        .sort((a, b) => (a.bracket_slot ?? 0) - (b.bracket_slot ?? 0)),
    }))
    .filter((round) => round.matches.length > 0),
)

/** Connecteurs : appariement classique (N→N/2) ou liaison 1:1 (barrages → quarts). */
function connectorMeta(fromCount: number, toCount: number) {
  if (toCount <= 0) return null
  if (fromCount === toCount * 2) {
    return { mode: 'pair' as const, count: toCount }
  }
  if (fromCount === toCount) {
    return { mode: 'direct' as const, count: toCount }
  }
  return null
}

const segments = computed((): BracketSegment[] => {
  const list = rounds.value
  return list.map((round, index) => {
    const next = list[index + 1]
    return {
      round,
      connector: next
        ? connectorMeta(round.matches.length, next.matches.length)
        : null,
    }
  })
})

function playerLabel(match: TournamentMatch, slot: 'player1' | 'player2') {
  if (slot === 'player1') {
    return match.player1_display_name || match.player1 || '—'
  }
  return match.player2_display_name || match.player2 || '—'
}

function playerName(match: TournamentMatch, slot: 'player1' | 'player2') {
  return slot === 'player1' ? match.player1 : match.player2
}

function armyId(match: TournamentMatch, slot: 'player1' | 'player2') {
  return slot === 'player1' ? match.player1_army_id : match.player2_army_id
}

function objectives(match: TournamentMatch, slot: 'player1' | 'player2') {
  return slot === 'player1'
    ? match.player1_objectives
    : match.player2_objectives
}

function scoreLabel(match: TournamentMatch, slot: 'player1' | 'player2') {
  if (!matchHasResult(match)) return null
  if (match.is_forfeit) {
    const winner = bracketMatchWinner(match)
    if (winner === slot) return 'V'
    if (winner) return 'F'
  }
  return String(objectives(match, slot))
}

function pillRowClass(
  match: TournamentMatch,
  slot: 'player1' | 'player2',
) {
  const winner = bracketMatchWinner(match)
  return {
    'bracket-pill-row--winner': winner === slot,
    'bracket-pill-row--loser':
      matchHasResult(match) && winner !== null && winner !== slot,
  }
}
</script>

<template>
  <div
    v-if="segments.length > 0"
    class="bracket-tree"
    :class="{ 'bracket-tree--compact': compact }"
  >
    <template v-for="(segment, index) in segments" :key="segment.round.phase">
      <div class="bracket-round">
        <p v-if="!compact" class="bracket-round-label">
          {{ segment.round.label }}
        </p>
        <div class="bracket-round-seeds">
          <div
            v-for="match in segment.round.matches"
            :key="match.id"
            class="bracket-seed"
          >
            <div
              class="bracket-pill"
              :class="{ 'bracket-pill--played': matchHasResult(match) }"
            >
              <div
                v-for="slot in (['player1', 'player2'] as const)"
                :key="slot"
                class="bracket-pill-row"
                :class="pillRowClass(match, slot)"
              >
                <ArmyLogo
                  v-if="armyId(match, slot)"
                  :army-id="armyId(match, slot)!"
                  class="bracket-pill-logo"
                />
                <span
                  v-else
                  class="bracket-pill-logo bracket-pill-logo--empty"
                />
                <PlayerLink
                  v-if="playerName(match, slot)"
                  :name="playerName(match, slot)!"
                  :display-name="
                    slot === 'player1'
                      ? match.player1_display_name
                      : match.player2_display_name
                  "
                  class="bracket-pill-name"
                />
                <span v-else class="bracket-pill-name bracket-pill-name--empty">
                  {{ playerLabel(match, slot) }}
                </span>
                <span
                  v-if="scoreLabel(match, slot) !== null"
                  class="bracket-pill-score"
                >
                  {{ scoreLabel(match, slot) }}
                </span>
                <span
                  v-if="bracketMatchWinner(match) === slot"
                  class="bracket-pill-marker"
                  aria-hidden="true"
                />
              </div>
            </div>
          </div>
        </div>
      </div>

      <div
        v-if="index < segments.length - 1 && segment.connector"
        class="bracket-connectors"
        :class="`bracket-connectors--${segment.connector.mode}`"
        aria-hidden="true"
      >
        <div
          v-for="n in segment.connector.count"
          :key="n"
          class="bracket-connector"
        />
      </div>
      <div
        v-else-if="index < segments.length - 1"
        class="bracket-connectors bracket-connectors--spacer"
        aria-hidden="true"
      />
    </template>
  </div>
</template>
