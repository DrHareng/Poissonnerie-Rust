<script setup lang="ts">
import ArmyLogo from '@/components/ArmyLogo.vue'
import MatchResultBadges from '@/components/MatchResultBadges.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
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
    v-if="mode === 'scores'"
    class="tournament-match-result-row"
    :class="{ 'tournament-match-result-row--compact': compact }"
  >
    <div class="flex min-w-0 items-center justify-end gap-1">
      <ArmyLogo
        v-if="player1ArmyId"
        :army-id="player1ArmyId"
        class="shrink-0"
      />
      <PlayerLink
        v-if="match.player1"
        :name="match.player1"
        :display-name="match.player1_display_name"
        class="min-w-0 truncate font-medium"
      />
      <span v-else class="font-medium text-muted-foreground">?</span>
    </div>
    <div class="tournament-match-result-badges shrink-0">
      <MatchResultBadges
        v-if="match.is_unplayed"
        :match="{
          player1_objectives: 0,
          player2_objectives: 0,
          player1_survivors: 0,
          player2_survivors: 0,
          outcome: null,
        }"
        pending-label="Non joué"
        :badge-min-ch="5"
      />
      <MatchResultBadges
        v-else-if="match.is_forfeit && !match.outcome"
        :match="{
          player1_objectives: 0,
          player2_objectives: 0,
          player1_survivors: 0,
          player2_survivors: 0,
          outcome: null,
        }"
        pending-label="Forfait"
        :badge-min-ch="5"
      />
      <MatchResultBadges
        v-else
        :match="match"
        :badge-min-ch="5"
      />
    </div>
    <div class="flex min-w-0 items-center gap-1">
      <ArmyLogo
        v-if="player2ArmyId"
        :army-id="player2ArmyId"
        class="shrink-0"
      />
      <PlayerLink
        v-if="match.player2"
        :name="match.player2"
        :display-name="match.player2_display_name"
        class="min-w-0 truncate font-medium"
      />
      <span v-else class="font-medium text-muted-foreground">?</span>
    </div>
  </div>

  <div
    v-else
    class="tournament-match-scoreboard"
    :class="{ 'tournament-match-scoreboard--compact': compact }"
  >
    <section
      class="player-match-panel"
      :class="{ 'player-match-panel--inline': mode === 'players' }"
    >
      <template v-if="mode === 'form'">
        <div class="flex items-center gap-2">
          <ArmyLogo
            v-if="player1ArmyId"
            :army-id="player1ArmyId"
            class="shrink-0"
          />
          <PlayerLink
            v-if="match.player1"
            :name="match.player1"
            :display-name="match.player1_display_name"
            class="font-medium"
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
        <ArmyLogo
          v-if="player1ArmyId"
          :army-id="player1ArmyId"
          class="shrink-0"
        />
        <PlayerLink
          v-if="match.player1"
          :name="match.player1"
          :display-name="match.player1_display_name"
          class="shrink-0 font-medium"
        />
        <span v-else class="shrink-0 font-medium text-muted-foreground">?</span>
      </template>
    </section>

    <div class="tournament-match-vs">VS</div>

    <section
      class="player-match-panel"
      :class="{ 'player-match-panel--inline': mode === 'players' }"
    >
      <template v-if="mode === 'form'">
        <div class="flex items-center gap-2">
          <ArmyLogo
            v-if="player2ArmyId"
            :army-id="player2ArmyId"
            class="shrink-0"
          />
          <PlayerLink
            v-if="match.player2"
            :name="match.player2"
            :display-name="match.player2_display_name"
            class="font-medium"
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
        <ArmyLogo
          v-if="player2ArmyId"
          :army-id="player2ArmyId"
          class="shrink-0"
        />
        <PlayerLink
          v-if="match.player2"
          :name="match.player2"
          :display-name="match.player2_display_name"
          class="shrink-0 font-medium"
        />
        <span v-else class="shrink-0 font-medium text-muted-foreground">?</span>
      </template>
    </section>
  </div>
</template>
