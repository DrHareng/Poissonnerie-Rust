<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { Plus } from '@lucide/vue'
import { fetchRanking, fetchRecentMatches } from '@/lib/api'
import type { MatchRecord, RankedPlayer } from '@/types/elo'
import RecentMatchesList from '@/components/RecentMatchesList.vue'
import RecordMatchCard from '@/components/RecordMatchCard.vue'
import { useAuth } from '@/composables/useAuth'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'

const PAGE_SIZE = 5

const { isAuthenticated, login } = useAuth()

const players = ref<RankedPlayer[]>([])
const matches = ref<MatchRecord[]>([])
const totalMatches = ref(0)
const page = ref(1)
const loadingPlayers = ref(true)
const loadingMatches = ref(true)
const apiOnline = ref(true)
const showForm = ref(false)

const totalPages = computed(() => Math.max(1, Math.ceil(totalMatches.value / PAGE_SIZE)))

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
    const response = await fetchRecentMatches(PAGE_SIZE, (page.value - 1) * PAGE_SIZE)
    matches.value = response.items
    totalMatches.value = response.total
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

function onPageChange(nextPage: number) {
  page.value = nextPage
}

function onRecorded() {
  showForm.value = false
  page.value = 1
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

watch(page, refreshMatches)

onMounted(refreshAll)
</script>

<template>
  <div class="page-stack">
    <section class="page-header">
      <div class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div class="space-y-2">
          <h1 class="page-title">Matchs</h1>
          <p class="page-description">
            Consultez les résultats enregistrés.
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

    <RecentMatchesList
      :matches="matches"
      :loading="loadingMatches"
      :page="page"
      :page-size="PAGE_SIZE"
      :total="totalMatches"
      :total-pages="totalPages"
      @page-change="onPageChange"
    />
  </div>
</template>
