<script setup lang="ts">
import { scoreTone } from '@/format'
import type { MatchCard } from '@/types'

defineProps<{
  matches: MatchCard[]
}>()
</script>

<template>
  <table class="sheet">
    <thead>
      <tr>
        <th></th>
        <th class="player">Joueur</th>
        <th></th>
        <th class="player">Joueur</th>
        <th></th>
      </tr>
    </thead>
    <tbody>
      <tr v-for="game in matches" :key="`${game.round}-${game.table ?? 'bye'}-${game.player1.id}`">
        <td>{{ game.bye ? 'Bye' : `T${game.table}` }}</td>
        <td class="player">
          <strong class="clip">{{ game.player1.name }}</strong>
        </td>
        <template v-if="game.bye">
          <td></td>
          <td class="player"></td>
          <td></td>
        </template>
        <template v-else>
          <td><span class="score-badge" :class="scoreTone(game, 1)">{{ game.p1_op }} - {{ game.p1_vp }}</span></td>
          <td class="player">
            <strong class="clip">{{ game.player2?.name }}</strong>
          </td>
          <td><span class="score-badge" :class="scoreTone(game, 2)">{{ game.p2_op }} - {{ game.p2_vp }}</span></td>
        </template>
      </tr>
    </tbody>
  </table>
</template>
