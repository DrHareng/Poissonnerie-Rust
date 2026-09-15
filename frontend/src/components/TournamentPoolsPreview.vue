<script setup lang="ts">
import PlayerLink from '@/components/PlayerLink.vue'
import type { Pool } from '@/types/elo'

withDefaults(
  defineProps<{
    pools: Pool[]
    compact?: boolean
  }>(),
  {
    compact: false,
  },
)
</script>

<template>
  <div
    class="grid gap-3"
    :class="compact ? 'sm:grid-cols-2' : 'md:grid-cols-2'"
  >
    <div
      v-for="pool in pools"
      :key="pool.id"
      class="rounded-lg border border-border/60 p-3 text-left"
    >
      <h3
        :class="
          compact
            ? 'mb-2 text-sm font-semibold'
            : 'mb-2 font-semibold'
        "
      >
        {{ pool.name }}
      </h3>
      <ul class="grid gap-1">
        <li
          v-for="player in pool.players"
          :key="player.player_name"
          class="truncate text-sm text-muted-foreground"
        >
          <PlayerLink
            :name="player.player_name"
            :display-name="player.player_display_name"
          />
        </li>
        <li
          v-if="pool.players.length === 0"
          class="text-sm text-muted-foreground"
        >
          Aucun joueur
        </li>
      </ul>
    </div>
  </div>
</template>
