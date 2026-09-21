<script setup lang="ts">
import { onMounted } from 'vue'
import ArmyLogo from '@/components/ArmyLogo.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import { Card, CardContent } from '@/components/ui/card'
import { useArmies } from '@/composables/useArmies'
import type { PoolPlayer } from '@/types/elo'

const { players } = defineProps<{
  players: PoolPlayer[]
}>()

const { ensureLoaded, getArmy } = useArmies()

onMounted(() => {
  void ensureLoaded()
})

function playerLabel(player: PoolPlayer) {
  return player.player_display_name?.trim() || player.player_name
}

function playerInitial(player: PoolPlayer) {
  const label = playerLabel(player)
  return label.charAt(0).toUpperCase() || '?'
}

function playerRatingLabel(player: PoolPlayer) {
  if (player.rating == null || Number.isNaN(player.rating)) return '—'
  return String(Math.round(player.rating))
}

function armyName(player: PoolPlayer) {
  return getArmy(player.army_id)?.name ?? '—'
}
</script>

<template>
  <div class="pool-player-cards">
    <Card
      v-for="player in players"
      :key="player.player_name"
      size="sm"
      class="pool-player-card overflow-visible"
    >
      <CardContent class="grid gap-1.5 py-0">
        <div class="pool-player-card-row">
          <img
            v-if="player.avatar_url"
            :src="player.avatar_url"
            :alt="playerLabel(player)"
            class="pool-player-card-avatar"
          />
          <span
            v-else
            class="pool-player-card-avatar-fallback"
            aria-hidden="true"
          >{{ playerInitial(player) }}</span>
          <PlayerLink
            :name="player.player_name"
            :display-name="player.player_display_name"
            class="pool-player-card-name"
          />
          <span class="pool-player-card-stat">ELO : {{ playerRatingLabel(player) }}</span>
        </div>
        <div class="pool-player-card-row pool-player-card-row--army">
          <span class="pool-player-card-army-logo">
            <ArmyLogo
              :army-id="player.army_id"
              size="sm"
            />
          </span>
          <span
            class="pool-player-card-army"
            :title="armyName(player)"
          >{{ armyName(player) }}</span>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
