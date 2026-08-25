<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { fetchArmyRanking } from '@/lib/api'
import type { RankedArmy } from '@/types/elo'
import ArmyRankingTable from '@/components/ArmyRankingTable.vue'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { classementTabs } from '@/lib/pageTitleTabs'

const router = useRouter()

const armies = ref<RankedArmy[]>([])
const loading = ref(true)
const apiOnline = ref(true)

async function refreshRanking() {
  loading.value = true
  try {
    armies.value = await fetchArmyRanking()
    apiOnline.value = true
  } catch (error) {
    apiOnline.value = false
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de charger le classement des sectorielles',
    )
  } finally {
    loading.value = false
  }
}

function openSectoriellePage(army: RankedArmy) {
  router.push({ name: 'sectorielle', params: { id: army.army_id } })
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
      <p class="page-description">
        Classement des factions jouées dans les parties enregistrées (win rate ou nombre de parties).
      </p>
    </section>

    <Alert v-if="!apiOnline" variant="destructive" class="neon-panel-accent">
      <AlertTitle>API indisponible</AlertTitle>
      <AlertDescription>
        Lancez le serveur Rust avec
        <code class="rounded bg-muted px-1 py-0.5">cargo run --bin poissonnerie-server</code>
        puis rechargez la page.
      </AlertDescription>
    </Alert>

    <ArmyRankingTable
      :armies="armies"
      :loading="loading"
      @select="openSectoriellePage"
    />
  </div>
</template>
