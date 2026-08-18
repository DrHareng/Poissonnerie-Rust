<script setup lang="ts">
import ArmyLogo from '@/components/ArmyLogo.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { matchPlayerScores } from '@/lib/tournamentMatchDisplay'
import type { TournamentMatchForm } from '@/components/TournamentMatchCard.vue'
import type { TournamentMatch } from '@/types/elo'

defineProps<{
  match: TournamentMatch
  mode: 'scores' | 'form' | 'players'
  form?: TournamentMatchForm
  player1ArmyId?: number
  player2ArmyId?: number
  compact?: boolean
}>()
</script>

<template>
  <div
    class="tournament-match-scoreboard"
    :class="{ 'tournament-match-scoreboard--compact': compact }"
  >
    <section
      class="player-match-panel"
      :class="{ 'player-match-panel--inline': mode === 'scores' || mode === 'players' }"
    >
      <template v-if="mode === 'form'">
        <div class="flex items-center gap-2">
          <PlayerLink
            v-if="match.player1"
            :name="match.player1"
            :display-name="match.player1_display_name"
            class="font-medium"
          />
          <ArmyLogo
            v-if="player1ArmyId"
            :army-id="player1ArmyId"
            class="shrink-0"
          />
        </div>
        <div class="grid gap-2">
          <Label :for="`match-${match.id}-p1-obj`">Points d'objectifs</Label>
          <Input
            :id="`match-${match.id}-p1-obj`"
            v-model.number="form!.p1"
            type="number"
            min="0"
            max="10"
          />
        </div>
        <div class="grid gap-2">
          <Label :for="`match-${match.id}-p1-surv`">Points de survivants</Label>
          <Input
            :id="`match-${match.id}-p1-surv`"
            v-model.number="form!.s1"
            type="number"
            min="0"
            max="300"
          />
        </div>
      </template>
      <template v-else>
        <PlayerLink
          v-if="match.player1"
          :name="match.player1"
          :display-name="match.player1_display_name"
          class="shrink-0 font-medium"
        />
        <span v-else class="shrink-0 font-medium text-muted-foreground">?</span>
        <ArmyLogo
          v-if="player1ArmyId"
          :army-id="player1ArmyId"
          class="shrink-0"
        />
        <span
          v-if="mode === 'scores'"
          class="tournament-match-scores tabular-nums"
        >
          {{ matchPlayerScores(match, 'player1').pt }} PT /
          {{ matchPlayerScores(match, 'player1').po }} PO /
          {{ matchPlayerScores(match, 'player1').ps }} PS
        </span>
      </template>
    </section>

    <div class="tournament-match-vs">VS</div>

    <section
      class="player-match-panel"
      :class="{ 'player-match-panel--inline': mode === 'scores' || mode === 'players' }"
    >
      <template v-if="mode === 'form'">
        <div class="flex items-center gap-2">
          <PlayerLink
            v-if="match.player2"
            :name="match.player2"
            :display-name="match.player2_display_name"
            class="font-medium"
          />
          <ArmyLogo
            v-if="player2ArmyId"
            :army-id="player2ArmyId"
            class="shrink-0"
          />
        </div>
        <div class="grid gap-2">
          <Label :for="`match-${match.id}-p2-obj`">Points d'objectifs</Label>
          <Input
            :id="`match-${match.id}-p2-obj`"
            v-model.number="form!.p2"
            type="number"
            min="0"
            max="10"
          />
        </div>
        <div class="grid gap-2">
          <Label :for="`match-${match.id}-p2-surv`">Points de survivants</Label>
          <Input
            :id="`match-${match.id}-p2-surv`"
            v-model.number="form!.s2"
            type="number"
            min="0"
            max="300"
          />
        </div>
      </template>
      <template v-else>
        <PlayerLink
          v-if="match.player2"
          :name="match.player2"
          :display-name="match.player2_display_name"
          class="shrink-0 font-medium"
        />
        <span v-else class="shrink-0 font-medium text-muted-foreground">?</span>
        <ArmyLogo
          v-if="player2ArmyId"
          :army-id="player2ArmyId"
          class="shrink-0"
        />
        <span
          v-if="mode === 'scores'"
          class="tournament-match-scores tabular-nums"
        >
          {{ matchPlayerScores(match, 'player2').pt }} PT /
          {{ matchPlayerScores(match, 'player2').po }} PO /
          {{ matchPlayerScores(match, 'player2').ps }} PS
        </span>
      </template>
    </section>
  </div>
</template>
