<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import FinalPanel from '@/components/FinalPanel.vue'
import PairingPanel from '@/components/PairingPanel.vue'
import PrepPanel from '@/components/PrepPanel.vue'
import RoundClock from '@/components/RoundClock.vue'
import RoundPanel from '@/components/RoundPanel.vue'
import StandingsTable from '@/components/StandingsTable.vue'
import { loadTournament, saveTournament } from '@/db'
import { dispatch, freshSeed, loadSectorials, openView } from '@/engine'
import { statusLabel, type PlayerCard, type Sectorial, type Tournament, type View } from '@/types'

const route = useRoute()
const tournament = ref<Tournament | null>(null)
const view = ref<View | null>(null)
const sectorials = ref<Sectorial[]>([])
const error = ref('')
const busy = ref(false)
const missing = ref(false)

const id = computed(() => String(route.params.id))

async function load() {
  error.value = ''
  missing.value = false
  const stored = await loadTournament(id.value)
  if (!stored) {
    missing.value = true
    tournament.value = null
    view.value = null
    return
  }
  tournament.value = stored
  view.value = await openView(stored)
}

onMounted(async () => {
  sectorials.value = await loadSectorials()
  await load()
})

watch(id, load)

async function run(action: object) {
  if (!tournament.value) return false
  error.value = ''
  busy.value = true
  try {
    const result = await dispatch(tournament.value, action)
    tournament.value = result.tournament
    view.value = result.view
    await saveTournament(result.tournament)
    return true
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Action impossible'
    return false
  } finally {
    busy.value = false
  }
}

async function saveMeta(payload: { name: string; rounds: number; tables: number; round_minutes: number }) {
  await run({ type: 'update_meta', ...payload })
}

async function start(payload: { name: string; rounds: number; tables: number; round_minutes: number }) {
  const saved = await run({ type: 'update_meta', ...payload })
  if (!saved) return
  await run({ type: 'start', seed: freshSeed() })
}

function askDrop(player: PlayerCard, bye: boolean) {
  const duringPairing = tournament.value?.status === 'pairing'
  const message = duringPairing
    ? `${player.name} quitte le tournoi. L'appariement sera recalculé.`
    : bye
      ? `${player.name} quitte la suite du tournoi. Le bye de cette ronde est conservé.`
      : `${player.name} déclare forfait. L'adversaire reçoit 4 TP, 0 OP et 0 VP.`
  if (!confirm(message)) return
  void run({ type: 'drop_player', player_id: player.id, seed: freshSeed() })
}

function askUndo(player: PlayerCard) {
  if (!confirm(`Annuler le forfait de ${player.name} ?`)) return
  void run({ type: 'undo_drop', player_id: player.id, seed: freshSeed() })
}

</script>

<template>
  <main class="shell">
    <header class="topbar">
      <router-link class="brand" to="/">
        <img src="/logo.png" alt="" />
        <div class="brand-text">
          <strong>Gestion de tournoi Infinity</strong>
          <span>Retour aux tournois</span>
        </div>
      </router-link>
      <span v-if="tournament" class="badge" :class="{ live: tournament.status !== 'preparation' && tournament.status !== 'finished' }">
        {{ statusLabel(tournament.status, tournament.round) }}
      </span>
    </header>

    <p v-if="missing" class="empty">Ce tournoi n'est pas sur cet appareil.</p>
    <p v-else-if="!tournament || !view" class="empty">Chargement…</p>

    <template v-else>
      <h1>{{ tournament.name }}</h1>
      <p v-if="error" class="banner error">{{ error }}</p>

      <PrepPanel
        v-if="tournament.status === 'preparation'"
        :tournament="tournament"
        :view="view"
        :sectorials="sectorials"
        :busy="busy"
        @save-meta="saveMeta"
        @add-player="run({ type: 'add_player', ...$event })"
        @remove-player="run({ type: 'remove_player', player_id: $event })"
        @start="start"
      />

      <PairingPanel
        v-else-if="tournament.status === 'pairing'"
        :view="view"
        :round="tournament.round"
        :rounds="tournament.rounds"
        :busy="busy"
        :sectorials="sectorials"
        @set-bye="run({ type: 'set_bye', player_id: $event, seed: freshSeed() })"
        @flag="(game, flagged) => run({ type: 'set_flag', player_a: game.player1.id, player_b: game.player2?.id ?? 0, flagged })"
        @regenerate="run({ type: 'regenerate', seed: freshSeed() })"
        @validate="run({ type: 'validate_pairing' })"
        @edit-players="run({ type: 'return_to_players' })"
        @drop="askDrop"
        @undo="askUndo"
      />

      <RoundPanel
        v-else-if="tournament.status === 'in_progress'"
        :rounds="view.rounds"
        :current-round="tournament.round"
        :total-rounds="tournament.rounds"
        :can-validate="view.can_validate_round"
        :busy="busy"
        :sectorials="sectorials"
        :save-result="(payload) => run({ type: 'set_result', ...payload })"
        @drop="askDrop"
        @undo="askUndo"
        @validate="run({ type: 'validate_round', seed: freshSeed() })"
      />

      <template v-else>
        <StandingsTable :rows="view.standings" :final-ranking="true" :sectorials="sectorials" />
        <FinalPanel :rounds="view.rounds" />
      </template>

      <div v-if="tournament.status === 'pairing' || tournament.status === 'in_progress'" class="stack" style="margin-top: 1rem">
        <StandingsTable :rows="view.standings" :final-ranking="false" :sectorials="sectorials" />
      </div>

      <RoundClock
        v-if="tournament.status === 'in_progress'"
        :tournament-id="tournament.id"
        :round="tournament.round"
        :minutes="tournament.round_minutes || 150"
        @duration="run({ type: 'set_round_minutes', minutes: $event })"
      />
    </template>
  </main>
</template>
