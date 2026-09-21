<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { BookOpen } from '@lucide/vue'
import { toast } from 'vue-sonner'
import { fetchRessources, updateRessources } from '@/lib/api'
import { pageTitle } from '@/lib/pageTitle'
import AdminContentEditor from '@/components/AdminContentEditor.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import TtsMapsTab from '@/components/TtsMapsTab.vue'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

type MapsLinksTabId = 'maps' | 'liens'

const tabs = [
  { id: 'maps' as const, label: 'Map TTS', routeName: 'maps' as const },
  { id: 'liens' as const, label: 'Liens', routeName: 'links' as const },
]

const route = useRoute()
const router = useRouter()
const { canEditContent } = useAdminEditMode()

const bodyMd = ref('')
const loading = ref(true)
const apiOnline = ref(true)
const liensLoaded = ref(false)
const currentMap = ref<{ slug: string; name: string } | null>(null)

const activeTab = computed<MapsLinksTabId>(() =>
  route.name === 'links' ? 'liens' : 'maps',
)

const activeTabLabel = computed(() => {
  if (activeTab.value === 'maps' && currentMap.value) {
    return currentMap.value.name
  }
  return tabs.find((tab) => tab.id === activeTab.value)?.label ?? 'Map TTS'
})

function setActiveTab(tab: MapsLinksTabId) {
  if (tab === 'maps') {
    router.replace({ name: 'maps', query: {} })
    return
  }
  currentMap.value = null
  router.replace({ name: 'links' })
}

async function loadLiens() {
  if (liensLoaded.value) return
  loading.value = true
  try {
    const content = await fetchRessources()
    bodyMd.value = content.body_md
    apiOnline.value = true
    liensLoaded.value = true
  } catch (error) {
    apiOnline.value = false
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de charger les liens',
    )
  } finally {
    loading.value = false
  }
}

async function save(payload: { body: string }) {
  const content = await updateRessources({ body_md: payload.body })
  bodyMd.value = content.body_md
  toast.success('Liens enregistrés')
}

onMounted(() => {
  if (activeTab.value === 'liens') {
    void loadLiens()
  }
})

watch(activeTab, (tab) => {
  document.title = pageTitle(tab === 'liens' ? 'Liens' : 'Map TTS')
  if (tab === 'liens') {
    void loadLiens()
  }
})
</script>

<template>
  <div class="page-stack">
    <nav class="page-title-tabs shrink-0" aria-label="Maps et liens">
      <div class="page-title-tabs-list">
        <h1 class="sr-only">{{ activeTabLabel }}</h1>
        <button
          v-for="tab in tabs"
          :key="tab.id"
          type="button"
          class="page-title-tab"
          :class="{
            'page-title-tab--active':
              !currentMap && activeTab === tab.id,
          }"
          :aria-current="
            !currentMap && activeTab === tab.id ? 'page' : undefined
          "
          @click="setActiveTab(tab.id)"
        >
          {{ tab.label }}
        </button>
        <RouterLink
          v-if="currentMap"
          :to="{ name: 'maps', query: { map: currentMap.slug } }"
          class="page-title-tab page-title-tab--detail page-title-tab--active"
          aria-current="page"
          :title="currentMap.name"
        >
          <span class="page-title-tab-detail">> {{ currentMap.name }}</span>
        </RouterLink>
      </div>
    </nav>

    <Alert v-if="!apiOnline && activeTab === 'liens'" variant="destructive" class="neon-panel-accent shrink-0">
      <AlertTitle>API indisponible</AlertTitle>
      <AlertDescription>
        Lancez le serveur Rust puis rechargez la page.
      </AlertDescription>
    </Alert>

    <TtsMapsTab
      v-else-if="activeTab === 'maps'"
      @map-change="currentMap = $event"
    />

    <p v-else-if="loading" class="shrink-0 text-sm text-muted-foreground">
      Chargement…
    </p>

    <Card
      v-else
      class="neon-panel page-panel-scroll flex min-h-0 flex-1 flex-col overflow-hidden"
    >
      <CardHeader class="shrink-0" :class="{ 'pr-24': canEditContent }">
        <CardTitle class="flex items-center gap-2">
          <BookOpen class="size-5 text-primary" />
          Liens
        </CardTitle>
      </CardHeader>
      <CardContent class="min-h-0 flex-1 overflow-y-auto">
        <AdminContentEditor
          :can-edit="canEditContent"
          :body="bodyMd"
          :rows="24"
          simple-markdown
          :persist="save"
        >
          <MarkdownContent v-if="bodyMd.trim()" :source="bodyMd" />
          <p v-else class="text-sm text-muted-foreground">
            Aucun lien pour l’instant.
          </p>
        </AdminContentEditor>
      </CardContent>
    </Card>
  </div>
</template>
