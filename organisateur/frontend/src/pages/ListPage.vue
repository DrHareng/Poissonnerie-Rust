<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import WheelPicker from '@/components/WheelPicker.vue'
import { forgetClock } from '@/clock'
import { deleteTournament, listTournaments, saveTournament, type TournamentListItem } from '@/db'
import { dispatch } from '@/engine'
import { statusLabel } from '@/types'

const router = useRouter()
const tournaments = ref<TournamentListItem[]>([])
const error = ref('')
const busy = ref(false)
const form = reactive({ name: '', rounds: 3, tables: 4 })

async function reload() {
  tournaments.value = await listTournaments()
}

onMounted(reload)

async function create() {
  error.value = ''
  busy.value = true
  try {
    const created = await dispatch(null, {
      type: 'create',
      id: crypto.randomUUID(),
      name: form.name,
      rounds: form.rounds,
      tables: form.tables,
    })
    await saveTournament(created.tournament)
    await router.push(`/t/${created.tournament.id}`)
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Création impossible'
  } finally {
    busy.value = false
  }
}

async function remove(item: TournamentListItem) {
  if (!confirm(`Supprimer « ${item.name} » ?`)) return
  await forgetClock(item.id)
  await deleteTournament(item.id)
  await reload()
}

function subtitle(item: TournamentListItem) {
  if (item.status === 'preparation') return `${item.players} inscrits`
  if (item.status === 'finished') return `${item.rounds} rondes`
  return `Ronde ${item.round} / ${item.rounds}`
}
</script>

<template>
  <main class="shell">
    <header class="topbar">
      <div class="brand">
        <img src="/logo.png" alt="" />
        <div class="brand-text">
          <strong>Organisateur de tournoi Infinity</strong>
          <span>Rondes suisses</span>
        </div>
      </div>
    </header>

    <form class="card stack" @submit.prevent="create">
      <h1>Nouveau tournoi</h1>
      <div class="fields">
        <label>
          <span>Nom</span>
          <input v-model="form.name" required maxlength="80" placeholder="Coupe du samedi" />
        </label>
        <div class="fields two">
          <WheelPicker v-model="form.rounds" :min="1" :max="12" label="Rondes" />
          <WheelPicker v-model="form.tables" :min="1" :max="40" label="Tables" />
        </div>
      </div>
      <p v-if="error" class="banner error">{{ error }}</p>
      <button class="btn primary" type="submit" :disabled="busy">Créer</button>
    </form>

    <section class="stack" style="margin-top: 1rem">
      <h2>Tournois</h2>
      <p v-if="tournaments.length === 0" class="empty">Aucun tournoi enregistré sur cet appareil.</p>
      <ul v-else class="list">
        <li v-for="item in tournaments" :key="item.id" class="card">
          <router-link :to="`/t/${item.id}`" class="person">
            <div>
              <strong>{{ item.name }}</strong>
              <div class="muted">{{ subtitle(item) }}</div>
            </div>
            <span class="badge" :class="{ live: item.status === 'in_progress' || item.status === 'pairing' }">
              {{ statusLabel(item.status, item.round) }}
            </span>
          </router-link>
          <button class="btn danger" type="button" @click.stop="remove(item)">Supprimer</button>
        </li>
      </ul>
    </section>
  </main>
</template>
