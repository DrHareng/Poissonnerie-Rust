<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { toast } from 'vue-sonner'
import { Plus } from '@lucide/vue'
import { fetchRanking, fetchRecentMatches } from '@/lib/api'
import type { MatchRecord, RankedPlayer } from '@/types/elo'
import RecentMatchesList from '@/components/RecentMatchesList.vue'
import RecordMatchCard from '@/components/RecordMatchCard.vue'
import { useAuth } from '@/composables/useAuth'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'

const { isAuthenticated, login } = useAuth()

const players = ref<RankedPlayer[]>([])
const matches = ref<MatchRecord[]>([])
const loadingPlayers = ref(true)
const loadingMatches = ref(true)
const apiOnline = ref(true)
const showForm = ref(false)

async function refreshPlayers() {
  loadingPlayers.value = true
  try {
    players.value = await fetchRanking()
    apiOnline.value = true
  } catch (error) {
    apiOnline.value = false
    toast.error(error instanceof Error ? error.message : 'Impossible de charger les joueurs')
  } finally {
    loadingPlayers.value = false
  }
}

async function refreshMatches() {
  loadingMatches.value = true
  try {
    matches.value = await fetchRecentMatches()
    apiOnline.value = true
  } catch (error) {
    apiOnline.value = false
    toast.error(error instanceof Error ? error.message : 'Impossible de charger les matchs')
  } finally {
    loadingMatches.value = false
  }
}

async function refreshAll() {
  await Promise.all([refreshPlayers(), refreshMatches()])
}

function onRecorded() {
  showForm.value = false
  refreshAll()
}

function onCancel() {
  showForm.value = false
}

function openForm() {
  if (!isAuthenticated.value) {
    toast.error('Connectez-vous avec Discord pour saisir un résultat.')
    login()
    return
  }
  showForm.value = true
}

onMounted(refreshAll)
</script>

<template>
  <div class="page-stack">
    <section class="page-header">
      <div class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div class="space-y-2">
          <h1 class="page-title">Matchs</h1>
          <p class="page-description">
            Consultez les derniers résultats enregistrés.
          </p>
        </div>
        <Button v-if="!showForm" @click="openForm">
          <Plus class="size-4" />
          Saisir un résultat
        </Button>
      </div>
    </section>

    <Alert v-if="!apiOnline" variant="destructive" class="neon-panel-accent">
      <AlertTitle>API indisponible</AlertTitle>
      <AlertDescription>
        Lancez le serveur Rust avec
        <code class="rounded bg-muted px-1 py-0.5">cargo run --bin poissonnerie-server</code>
        puis rechargez la page.
      </AlertDescription>
    </Alert>

    <RecordMatchCard
      v-if="showForm"
      :players="players"
      :loading="loadingPlayers"
      @recorded="onRecorded"
      @cancel="onCancel"
    />

    <RecentMatchesList :matches="matches" :loading="loadingMatches" />
  </div>
</template>
