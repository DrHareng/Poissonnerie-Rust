<script setup lang="ts">
import { computed } from 'vue'
import { Badge } from '@/components/ui/badge'
import { matchScoreLabel } from '@/lib/matchResultBadges'

/** Champs minimaux pour les pastilles (match Elo ou tournoi). */
export type MatchResultBadgeSource = {
  player1_objectives: number
  player2_objectives: number
  player1_survivors: number
  player2_survivors: number
  outcome?: string | null
  status?: string
  awaiting_confirmation?: boolean
}

const props = defineProps<{
  match: MatchResultBadgeSource
  /** Largeur mini commune des badges score (en `ch`), partagée sur une colonne. */
  badgeMinCh?: number
  /** Met en évidence la défaite (badge adversaire rouge). */
  emphasizeDefeat?: boolean
  /** Libellé si pas encore de résultat (défaut : En cours). */
  pendingLabel?: string
}>()

const badgeStyle = computed(() =>
  props.badgeMinCh != null
    ? { minWidth: `${props.badgeMinCh}ch` }
    : undefined,
)

const badgePairGapRem = 0.5

const pairStyle = computed(() =>
  props.badgeMinCh != null
    ? {
        // minWidth (pas width) : les badges peuvent dépasser badgeMinCh
        // (ex. « 10 - 10 ») sans se compresser / se superposer.
        minWidth: `calc(${props.badgeMinCh * 2}ch + ${badgePairGapRem}rem)`,
      }
    : undefined,
)

const inProgressStyle = computed(() =>
  props.badgeMinCh != null
    ? {
        minWidth: `calc(${props.badgeMinCh * 2}ch + ${badgePairGapRem}rem)`,
      }
    : undefined,
)

const showPending = computed(
  () =>
    props.match.awaiting_confirmation
    || props.match.status === 'in_progress'
    || !props.match.outcome,
)

const pendingText = computed(() => {
  if (props.pendingLabel) return props.pendingLabel
  if (props.match.awaiting_confirmation) return 'À confirmer'
  return 'En cours'
})

function badgeVariant(
  match: MatchResultBadgeSource,
  player: 'player1' | 'player2',
): 'default' | 'secondary' | 'outline' | 'destructive' {
  if (!match.outcome || match.status === 'in_progress' || match.awaiting_confirmation) {
    return 'secondary'
  }
  if (match.outcome === 'draw') {
    return 'secondary'
  }
  if (player === 'player1' && match.outcome === 'player1_win') {
    return 'default'
  }
  if (player === 'player2' && match.outcome === 'player2_win') {
    return props.emphasizeDefeat ? 'outline' : 'default'
  }
  return 'outline'
}

function badgeClass(match: MatchResultBadgeSource, player: 'player1' | 'player2') {
  if (
    props.emphasizeDefeat
    && match.outcome === 'player2_win'
    && player === 'player2'
  ) {
    return 'match-result-badge--loss'
  }
  return undefined
}

function scoreLabel(objectives: number, survivors: number) {
  return matchScoreLabel(objectives, survivors)
}
</script>

<template>
  <div
    v-if="showPending"
    class="mx-auto flex justify-center text-center"
    :style="pairStyle"
  >
    <Badge
      variant="secondary"
      class="justify-center tabular-nums"
      :style="inProgressStyle"
    >
      {{ pendingText }}
    </Badge>
  </div>
  <div
    v-else
    class="mx-auto flex items-center justify-center gap-2"
    :class="badgeMinCh == null ? 'w-36' : undefined"
    :style="pairStyle"
  >
    <Badge
      :variant="badgeVariant(match, 'player1')"
      :class="[
        'shrink-0 justify-center tabular-nums',
        badgeClass(match, 'player1'),
        badgeMinCh == null ? 'ml-auto' : undefined,
      ]"
      :style="badgeStyle"
    >
      {{ scoreLabel(match.player1_objectives, match.player1_survivors) }}
    </Badge>
    <Badge
      :variant="badgeVariant(match, 'player2')"
      :class="[
        'shrink-0 justify-center tabular-nums',
        badgeClass(match, 'player2'),
        badgeMinCh == null ? 'mr-auto' : undefined,
      ]"
      :style="badgeStyle"
    >
      {{ scoreLabel(match.player2_objectives, match.player2_survivors) }}
    </Badge>
  </div>
</template>
