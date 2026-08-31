<script setup lang="ts">
import { ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { fetchArmyListMatches } from '@/lib/api'
import type { MatchRecord } from '@/types/elo'
import RecentMatchesList from '@/components/RecentMatchesList.vue'

const props = defineProps<{
  listId: number
}>()

const MATCHES_PAGE_SIZE = 5

const matches = ref<MatchRecord[]>([])
const loading = ref(true)
const page = ref(1)

async function loadMatches() {
  loading.value = true
  try {
    matches.value = await fetchArmyListMatches(props.listId)
    page.value = 1
  } catch (error) {
    matches.value = []
    page.value = 1
    toast.error(
      error instanceof Error ? error.message : 'Impossible de charger les matchs',
    )
  } finally {
    loading.value = false
  }
}

watch(
  () => props.listId,
  () => {
    void loadMatches()
  },
  { immediate: true },
)
</script>

<template>
  <div class="border-t border-border bg-muted/20 px-3 py-4">
    <div class="mb-3">
      <p class="text-sm font-semibold leading-none">Historique des parties</p>
      <p class="mt-1 text-xs text-muted-foreground">
        Matchs joués avec cette liste, du plus récent au plus ancien.
      </p>
    </div>
    <RecentMatchesList
      bare
      client-side
      :matches="matches"
      :loading="loading"
      :page="page"
      :page-size="MATCHES_PAGE_SIZE"
      :perspective-army-list-id="listId"
      empty-message="Aucune partie enregistrée avec cette liste."
      @page-change="page = $event"
    />
  </div>
</template>
