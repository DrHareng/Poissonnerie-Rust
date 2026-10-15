<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import DurationPicker from '@/components/DurationPicker.vue'
import SectorialPicker from '@/components/SectorialPicker.vue'
import { joinMinutes, splitMinutes } from '@/clock'
import { armyLogoUrl } from '@/format'
import type { Sectorial, Tournament, View } from '@/types'

const props = defineProps<{
  tournament: Tournament
  view: View
  sectorials: Sectorial[]
  busy: boolean
}>()

const emit = defineEmits<{
  'save-meta': [payload: { name: string; rounds: number; tables: number; round_minutes: number }]
  'add-player': [payload: { name: string; sectorial_id: number }]
  'remove-player': [playerId: number]
  start: [payload: { name: string; rounds: number; tables: number; round_minutes: number }]
}>()

const initialDuration = splitMinutes(props.tournament.round_minutes ?? 150)
const meta = reactive({
  name: props.tournament.name,
  rounds: props.tournament.rounds,
  tables: props.tournament.tables,
  hours: initialDuration.hours,
  minutes: initialDuration.minutes,
})

const player = reactive({
  name: '',
  sectorial_id: props.sectorials[0]?.id ?? 0,
})

const roundMinutes = computed(() => joinMinutes(meta.hours, meta.minutes))

function payload() {
  return {
    name: meta.name,
    rounds: meta.rounds,
    tables: meta.tables,
    round_minutes: roundMinutes.value,
  }
}

watch(
  () => props.tournament,
  (tournament) => {
    const duration = splitMinutes(tournament.round_minutes ?? 150)
    meta.name = tournament.name
    meta.rounds = tournament.rounds
    meta.tables = tournament.tables
    meta.hours = duration.hours
    meta.minutes = duration.minutes
  },
)

function sectorial(id: number) {
  return props.sectorials.find((item) => item.id === id)
}

function logo(id: number) {
  const slug = sectorial(id)?.slug
  return slug ? armyLogoUrl(slug) : ''
}

function submitPlayer() {
  if (!player.name.trim() || !player.sectorial_id) return
  emit('add-player', { name: player.name, sectorial_id: player.sectorial_id })
  player.name = ''
}

function askRemove(id: number, name: string) {
  if (!confirm(`Retirer ${name} ?`)) return
  emit('remove-player', id)
}
</script>

<template>
  <section class="stack">
    <form class="card" @submit.prevent="emit('save-meta', payload())">
      <div class="spread record-row">
        <h2>Tournoi</h2>
        <button class="btn" type="submit" :disabled="busy || !roundMinutes">Enregistrer</button>
      </div>
      <div class="stack">
        <div class="meta-line">
          <label>
            <span>Nom</span>
            <input v-model="meta.name" required maxlength="80" />
          </label>
          <label>
            <span>Tables</span>
            <input v-model.number="meta.tables" type="number" min="1" max="40" required />
          </label>
        </div>
        <div class="meta-line halves">
          <label>
            <span>Rondes</span>
            <input v-model.number="meta.rounds" type="number" min="1" max="12" required />
          </label>
          <DurationPicker v-model:hours="meta.hours" v-model:minutes="meta.minutes" label="Durée" />
        </div>
      </div>
      <p v-if="tournament.players.length >= 2" class="muted">
        Minimum {{ view.min_tables }} table{{ view.min_tables > 1 ? 's' : '' }} pour {{ tournament.players.length }} inscrits.
      </p>
    </form>

    <form class="card" @submit.prevent="submitPlayer">
      <h2>Ajouter un joueur</h2>
      <div class="player-add">
        <label>
          <span>Nom</span>
          <input v-model="player.name" required maxlength="40" />
        </label>
        <label>
          <span>Sectorielle</span>
          <SectorialPicker v-model="player.sectorial_id" :sectorials="sectorials" />
        </label>
        <button class="btn primary plus" type="submit" :disabled="busy" aria-label="Ajouter un joueur">+</button>
      </div>
    </form>

    <section class="card">
      <h2>Inscrits</h2>
      <p v-if="tournament.players.length === 0" class="empty">Aucun inscrit.</p>
      <ul v-else class="list">
        <li v-for="entrant in tournament.players" :key="entrant.id" class="person">
          <div class="player-line">
            <img
              v-if="logo(entrant.sectorial_id)"
              class="army-logo"
              :src="logo(entrant.sectorial_id)"
              alt=""
              :title="sectorial(entrant.sectorial_id)?.name"
            />
            <strong class="clip">{{ entrant.name }}</strong>
          </div>
          <button class="btn danger compact" type="button" @click="askRemove(entrant.id, entrant.name)">
            Retirer
          </button>
        </li>
      </ul>
    </section>

    <p v-if="view.start_reason" class="banner warn">{{ view.start_reason }}</p>
    <button
      class="btn primary"
      type="button"
      :disabled="busy || !view.can_start || !roundMinutes"
      @click="emit('start', payload())"
    >
      Démarrer le tournoi
    </button>
  </section>
</template>
