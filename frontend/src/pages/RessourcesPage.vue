<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { BookOpen } from '@lucide/vue'
import { toast } from 'vue-sonner'
import { fetchRessources, updateRessources } from '@/lib/api'
import { pageTitle } from '@/lib/pageTitle'
import { autreTabs, ttsTabs } from '@/lib/pageTitleTabs'
import AdminContentEditor from '@/components/AdminContentEditor.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import ContentUpdates from '@/components/ContentUpdates.vue'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import RecentMapUpdates from '@/components/RecentMapUpdates.vue'
import TtsMapsTab from '@/components/TtsMapsTab.vue'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

type MapsLinksTabId = 'maps' | 'liens' | 'applis'

const organisateurApkUrl = `${import.meta.env.BASE_URL}organisateur.apk`

const androidApps = [
  {
    name: 'Gestion de tournoi Infinity',
    description:
      'Version 1.0. Le téléphone demande d’autoriser l’installation depuis le navigateur.',
    href: organisateurApkUrl,
    download: 'Gestion-de-tournoi-Infinity.apk',
  },
]

const tabTitles: Record<MapsLinksTabId, string> = {
  maps: 'Maps',
  liens: 'Liens',
  applis: 'Applications',
}

const route = useRoute()
const { canEditContent } = useAdminEditMode()

const bodyMd = ref('')
const loading = ref(true)
const apiOnline = ref(true)
const liensLoaded = ref(false)
const currentMap = ref<{ id: number; slug: string; name: string } | null>(null)
const updatesVisible = ref(false)
const updatesRefresh = ref(0)

const activeTab = computed<MapsLinksTabId>(() => {
  if (route.name === 'links') return 'liens'
  if (route.name === 'applis') return 'applis'
  return 'maps'
})

const pageTabs = computed(() =>
  activeTab.value === 'maps' ? ttsTabs : autreTabs,
)

const pageTabsAria = computed(() =>
  activeTab.value === 'maps' ? 'TTS' : 'Autre',
)

const mapCurrent = computed(() =>
  activeTab.value === 'maps' && currentMap.value
    ? {
        label: currentMap.value.name,
        title: currentMap.value.name,
        to: { name: 'maps' as const, query: { map: currentMap.value.slug } },
      }
    : null,
)

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
  document.title = pageTitle(tabTitles[tab])
  if (tab !== 'maps') {
    currentMap.value = null
  }
  if (tab === 'liens') {
    void loadLiens()
  }
})
</script>

<template>
  <div class="page-stack">
    <PageTitleTabs
      class="shrink-0"
      :class="{ 'page-title-tabs--with-meta': updatesVisible && activeTab === 'maps' }"
      :tabs="pageTabs"
      :ariaLabel="pageTabsAria"
      :current="mapCurrent"
    >
      <template v-if="activeTab === 'maps'" #actions>
        <div v-show="updatesVisible">
          <RecentMapUpdates
            v-if="!currentMap"
            :refresh-key="updatesRefresh"
            @visible="updatesVisible = $event"
          />
          <ContentUpdates
            v-else
            kind="map"
            :map-id="currentMap.id"
            :refresh-key="updatesRefresh"
            @visible="updatesVisible = $event"
          />
        </div>
      </template>
    </PageTitleTabs>

    <Alert v-if="!apiOnline && activeTab === 'liens'" variant="destructive" class="neon-panel-accent shrink-0">
      <AlertTitle>API indisponible</AlertTitle>
      <AlertDescription>
        Lancez le serveur Rust puis rechargez la page.
      </AlertDescription>
    </Alert>

    <TtsMapsTab
      v-else-if="activeTab === 'maps'"
      @map-change="currentMap = $event"
      @updates-change="updatesRefresh += 1"
    />

    <div
      v-else-if="activeTab === 'applis'"
      class="grid min-h-0 flex-1 content-start gap-4 overflow-y-auto"
    >
      <Card v-for="app in androidApps" :key="app.name" class="neon-panel">
        <CardHeader class="pb-2">
          <CardTitle class="flex items-center gap-2">
            <span>{{ app.name }}</span>
            <svg
              viewBox="0 0 24 24"
              class="size-5 shrink-0 text-[#3DDC84]"
              aria-label="Android"
              role="img"
            >
              <path
                fill="currentColor"
                d="M6.38 3.97A8.96 8.96 0 0 1 12 2c2.1 0 4.03.72 5.55 1.93l1.22-1.22a1 1 0 0 1 1.42 1.42l-1.13 1.12A8.96 8.96 0 0 1 21 11v1h1a1 1 0 0 1 1 1v2a1 1 0 0 1-1 1h-1v4a2 2 0 0 1-2 2h-1v1a1 1 0 0 1-1 1h-2a1 1 0 0 1-1-1v-1h-2v1a1 1 0 0 1-1 1H8a1 1 0 0 1-1-1v-1H6a2 2 0 0 1-2-2v-4H3a1 1 0 0 1-1-1v-2a1 1 0 0 1 1-1h1v-1c0-1.9.59-3.66 1.59-5.1L4.4 4.17a1 1 0 1 1 1.42-1.42l.56.52zM8.5 9.25a1.25 1.25 0 1 0 0-2.5 1.25 1.25 0 0 0 0 2.5zm7 0a1.25 1.25 0 1 0 0-2.5 1.25 1.25 0 0 0 0 2.5z"
              />
            </svg>
          </CardTitle>
        </CardHeader>
        <CardContent class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <p class="text-sm text-muted-foreground">{{ app.description }}</p>
          <Button as="a" :href="app.href" :download="app.download" class="shrink-0">
            Télécharger
          </Button>
        </CardContent>
      </Card>
    </div>

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
