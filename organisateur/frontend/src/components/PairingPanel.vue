<script setup lang="ts">
import { computed } from 'vue'
import PlayerLine from '@/components/PlayerLine.vue'
import type { MatchCard, PlayerCard, Sectorial, View } from '@/types'

const props = defineProps<{
  view: View
  round: number
  rounds: number
  busy: boolean
  sectorials: Sectorial[]
}>()

const emit = defineEmits<{
  'set-bye': [playerId: number]
  flag: [game: MatchCard, flagged: boolean]
  regenerate: []
  validate: []
  drop: [player: PlayerCard, bye: boolean]
  undo: [player: PlayerCard]
  'edit-players': []
}>()

const current = computed(() => props.view.rounds.find((round) => round.provisional))
const selectedBye = computed(
  () => props.view.bye_options.find((option) => option.selected)?.player_id ?? '',
)

function onFlag(game: MatchCard, event: Event) {
  const input = event.target as HTMLInputElement
  emit('flag', game, input.checked)
}
</script>

<template>
  <section class="stack">
    <h2>Appariement — ronde {{ round }} / {{ rounds }}</h2>
    <p v-for="warning in view.warnings" :key="warning" class="banner warn">{{ warning }}</p>

    <label v-if="view.bye_required" class="card">
      <span>Bye</span>
      <select
        :value="selectedBye"
        :disabled="busy"
        @change="emit('set-bye', Number(($event.target as HTMLSelectElement).value))"
      >
        <option v-for="option in view.bye_options" :key="option.player_id" :value="option.player_id">
          {{ option.name }}
        </option>
      </select>
      <span class="muted">Par défaut, le dernier du classement qui n'a pas encore eu de bye.</span>
    </label>

    <article v-for="game in current?.matches ?? []" :key="game.player1.id" class="card match">
      <div class="spread">
        <strong>{{ game.bye ? 'Bye' : `Table ${game.table}` }}</strong>
        <label v-if="!game.bye" class="check">
          <input
            type="checkbox"
            :checked="game.flagged"
            :disabled="busy"
            @change="onFlag(game, $event)"
          />
          Problématique
        </label>
      </div>
      <PlayerLine :player="game.player1" :sectorials="sectorials">
        <button
          v-if="game.player1.can_undo_drop"
          class="btn compact"
          type="button"
          @click="emit('undo', game.player1)"
        >
          Annuler
        </button>
        <button
          v-else-if="!game.player1.dropped"
          class="btn compact danger"
          type="button"
          @click="emit('drop', game.player1, game.bye)"
        >
          Forfait
        </button>
      </PlayerLine>
      <PlayerLine v-if="game.player2" :player="game.player2" :sectorials="sectorials">
        <button
          v-if="game.player2.can_undo_drop"
          class="btn compact"
          type="button"
          @click="emit('undo', game.player2)"
        >
          Annuler
        </button>
        <button
          v-else-if="!game.player2.dropped"
          class="btn compact danger"
          type="button"
          @click="emit('drop', game.player2, false)"
        >
          Forfait
        </button>
      </PlayerLine>
    </article>

    <section v-if="view.recall.length" class="card stack">
      <h2>Forfaits</h2>
      <PlayerLine v-for="player in view.recall" :key="player.id" :player="player" :sectorials="sectorials">
        <button class="btn compact" type="button" :disabled="busy" @click="emit('undo', player)">
          Annuler
        </button>
      </PlayerLine>
    </section>

    <div class="row actions">
      <button
        v-if="round === 1"
        class="btn"
        type="button"
        :disabled="busy"
        @click="emit('edit-players')"
      >
        Annuler
      </button>
      <button class="btn" type="button" :disabled="busy" @click="emit('regenerate')">
        Changer
      </button>
      <button
        class="btn primary"
        type="button"
        :disabled="busy || !view.can_validate_pairing"
        @click="emit('validate')"
      >
        Valider
      </button>
    </div>
  </section>
</template>
