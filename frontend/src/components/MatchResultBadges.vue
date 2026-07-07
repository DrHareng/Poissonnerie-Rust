<script setup lang="ts">
import type { MatchRecord } from '@/types/elo'
import { Badge } from '@/components/ui/badge'

defineProps<{
  match: MatchRecord
}>()

function badgeVariant(
  match: MatchRecord,
  player: 'player1' | 'player2',
): 'default' | 'secondary' | 'outline' {
  if (match.outcome === 'draw') {
    return 'secondary'
  }
  if (player === 'player1' && match.outcome === 'player1_win') {
    return 'default'
  }
  if (player === 'player2' && match.outcome === 'player2_win') {
    return 'default'
  }
  return 'outline'
}

function scoreLabel(objectives: number, survivors: number) {
  return `${objectives} - ${survivors}`
}
</script>

<template>
  <div class="mx-auto grid w-36 grid-cols-2 gap-2">
    <Badge :variant="badgeVariant(match, 'player1')" class="justify-self-end tabular-nums">
      {{ scoreLabel(match.player1_objectives, match.player1_survivors) }}
    </Badge>
    <Badge :variant="badgeVariant(match, 'player2')" class="justify-self-start tabular-nums">
      {{ scoreLabel(match.player2_objectives, match.player2_survivors) }}
    </Badge>
  </div>
</template>
