<script setup lang="ts">
import { computed } from 'vue'
import type { MatchRecord } from '@/types/elo'
import { Badge } from '@/components/ui/badge'

const props = defineProps<{
  match: MatchRecord
  /** Largeur mini commune des badges score (en `ch`), partagée sur une colonne. */
  badgeMinCh?: number
  /** Met en évidence la défaite (badge adversaire rouge). */
  emphasizeDefeat?: boolean
}>()

const badgeStyle = computed(() =>
  props.badgeMinCh != null
    ? { minWidth: `${props.badgeMinCh}ch` }
    : undefined,
)

const pairStyle = computed(() =>
  props.badgeMinCh != null
    ? { width: `calc(${props.badgeMinCh * 2}ch + 0.25rem)` }
    : undefined,
)

const inProgressStyle = computed(() =>
  props.badgeMinCh != null
    ? { minWidth: `calc(${props.badgeMinCh * 2}ch + 0.25rem)` }
    : undefined,
)

function badgeVariant(
  match: MatchRecord,
  player: 'player1' | 'player2',
): 'default' | 'secondary' | 'outline' | 'destructive' {
  if (!match.outcome || match.status === 'in_progress') {
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

function badgeClass(match: MatchRecord, player: 'player1' | 'player2') {
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
  return `${objectives} - ${survivors}`
}
</script>

<template>
  <div
    v-if="match.status === 'in_progress' || !match.outcome"
    class="mx-auto flex justify-center text-center"
    :style="pairStyle"
  >
    <Badge
      variant="secondary"
      class="justify-center tabular-nums"
      :style="inProgressStyle"
    >
      En cours
    </Badge>
  </div>
  <div
    v-else
    class="mx-auto grid grid-cols-2 gap-1"
    :class="badgeMinCh == null ? 'w-36 gap-2' : undefined"
    :style="pairStyle"
  >
    <Badge
      :variant="badgeVariant(match, 'player1')"
      :class="[
        'justify-center tabular-nums',
        badgeClass(match, 'player1'),
        badgeMinCh == null ? 'justify-self-end' : undefined,
      ]"
      :style="badgeStyle"
    >
      {{ scoreLabel(match.player1_objectives, match.player1_survivors) }}
    </Badge>
    <Badge
      :variant="badgeVariant(match, 'player2')"
      :class="[
        'justify-center tabular-nums',
        badgeClass(match, 'player2'),
        badgeMinCh == null ? 'justify-self-start' : undefined,
      ]"
      :style="badgeStyle"
    >
      {{ scoreLabel(match.player2_objectives, match.player2_survivors) }}
    </Badge>
  </div>
</template>
