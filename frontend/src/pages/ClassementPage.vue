<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { Plus, Search } from '@lucide/vue'
import { toast } from 'vue-sonner'
import { fetchRanking } from '@/lib/api'
import type { RankedPlayer } from '@/types/elo'
import AddPlayerCard from '@/components/AddPlayerCard.vue'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import RankingTable from '@/components/RankingTable.vue'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { useAuth } from '@/composables/useAuth'
import { classementTabs } from '@/lib/pageTitleTabs'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

const router = useRouter()
const { isAdmin } = useAuth()

const players = ref<RankedPlayer[]>([])
const loading = ref(true)
const apiOnline = ref(true)
const searchQuery = ref('')
const showForm = ref(false)

async function refreshRanking() {
  loading.value = true
  try {
    players.value = await fetchRanking()
    apiOnline.value = true
  } catch (error) {
    apiOnline.value = false
    toast.error(error instanceof Error ? error.message : 'Impossible de charger le classement')
  } finally {
    loading.value = false
  }
}

function openPlayerPage(player: RankedPlayer) {
  router.push({ name: 'joueur', params: { name: player.name } })
}

function onPlayerCreated() {
  showForm.value = false
  refreshRanking()
}

function onCancel() {
  showForm.value = false
}

onMounted(refreshRanking)
</script>

<template>
  <div class="page-stack">
    <PageTitleTabs
      :tabs="classementTabs"
      ariaLabel="Sections du classement"
    />

    <section class="page-header">
      <div class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <p class="page-description">
          Consultez le classement ELO et ajoutez de nouveaux participants.
        </p>
        <div class="flex flex-col gap-3 sm:flex-row sm:items-center shrink-0">
          <div class="relative w-full sm:w-64">
            <Search
              class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
            />
            <Input
              v-model="searchQuery"
              placeholder="rechercher par nom"
              autocomplete="off"
              class="pl-8"
            />
          </div>
          <Button v-if="isAdmin && !showForm" @click="showForm = true">
            <Plus class="size-4" />
            Nouveau joueur
          </Button>
        </div>
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

    <AddPlayerCard
      v-if="isAdmin && showForm"
      @created="onPlayerCreated"
      @cancel="onCancel"
    />

    <RankingTable
      :players="players"
      :loading="loading"
      :search-query="searchQuery"
      @select="openPlayerPage"
    />
  </div>
</template>
