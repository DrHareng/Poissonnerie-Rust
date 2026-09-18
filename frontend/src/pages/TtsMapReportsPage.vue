<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { CircleAlert, Trash2 } from '@lucide/vue'
import { toast } from 'vue-sonner'
import {
  deleteTtsMapReport,
  fetchTtsMapReports,
} from '@/lib/api'
import { withBase } from '@/lib/basePath'
import { pageTitle } from '@/lib/pageTitle'
import { adminTabs } from '@/lib/pageTitleTabs'
import { refreshTtsMapReportCount } from '@/composables/useTtsMapReportCount'
import { useAuth } from '@/composables/useAuth'
import type { TtsMapReport } from '@/types/elo'
import ImageViewer, {
  type ImageViewerItem,
} from '@/components/ImageViewer.vue'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

const { isAdmin, loading: authLoading, initialized } = useAuth()

const reports = ref<TtsMapReport[]>([])
const loading = ref(true)
const imageViewerOpen = ref(false)
const imageViewerItems = ref<ImageViewerItem[]>([])

function formatDate(timestamp: number) {
  return new Intl.DateTimeFormat('fr-FR', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(timestamp * 1000))
}

async function load() {
  if (!isAdmin.value) {
    reports.value = []
    loading.value = false
    return
  }
  loading.value = true
  try {
    reports.value = await fetchTtsMapReports()
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de charger les signalements',
    )
  } finally {
    loading.value = false
  }
}

function openImage(report: TtsMapReport) {
  if (!report.image_url) return
  imageViewerItems.value = [
    {
      src: withBase(report.image_url),
      alt: report.image_filename ?? 'Capture',
      caption: report.map_name,
    },
  ]
  imageViewerOpen.value = true
}

async function removeReport(report: TtsMapReport) {
  if (!window.confirm(`Supprimer le signalement sur « ${report.map_name} » ?`)) {
    return
  }
  try {
    await deleteTtsMapReport(report.id)
    reports.value = reports.value.filter((item) => item.id !== report.id)
    toast.success('Signalement supprimé')
    void refreshTtsMapReportCount()
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de supprimer le signalement',
    )
  }
}

onMounted(() => {
  document.title = pageTitle('Signalements TTS')
  void load()
})

watch([initialized, isAdmin], ([ready, admin]) => {
  if (ready && admin) void load()
})
</script>

<template>
  <div class="page-stack">
    <PageTitleTabs ariaLabel="Administration" :tabs="adminTabs" />

    <div v-if="authLoading || !initialized" class="text-sm text-muted-foreground">
      Chargement...
    </div>

    <Alert v-else-if="!isAdmin" variant="destructive" class="neon-panel-accent">
      <AlertTitle>Accès réservé</AlertTitle>
      <AlertDescription>
        Cette page n’est disponible que pour les administrateurs.
      </AlertDescription>
    </Alert>

    <template v-else>
      <p v-if="loading" class="text-sm text-muted-foreground">Chargement…</p>
      <p v-else-if="reports.length === 0" class="text-sm text-muted-foreground">
        Aucun signalement pour l’instant.
      </p>
      <div v-else class="grid gap-3 pb-4">
        <Card v-for="report in reports" :key="report.id" class="neon-panel">
          <CardHeader class="flex flex-row items-start justify-between gap-3 space-y-0">
            <div class="min-w-0 space-y-1">
              <CardTitle class="flex items-center gap-2 text-base">
                <CircleAlert class="size-4 shrink-0 text-primary" />
                <RouterLink
                  :to="{ name: 'ressources', query: { map: report.map_slug } }"
                  class="truncate text-primary hover:underline"
                >
                  {{ report.map_name }}
                </RouterLink>
              </CardTitle>
              <CardDescription>
                Signalé par {{ report.reporter_display_name }}
                · {{ formatDate(report.created_at) }}
              </CardDescription>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              @click="removeReport(report)"
            >
              <Trash2 class="size-3.5" />
              Supprimer
            </Button>
          </CardHeader>
          <CardContent class="space-y-3">
            <p class="whitespace-pre-wrap text-sm leading-relaxed">
              {{ report.description }}
            </p>
            <button
              v-if="report.image_url"
              type="button"
              class="block max-w-md overflow-hidden rounded-md border border-border"
              @click="openImage(report)"
            >
              <img
                :src="withBase(report.image_url)"
                :alt="report.image_filename ?? 'Capture'"
                class="max-h-64 w-full object-contain"
              />
            </button>
          </CardContent>
        </Card>
      </div>
    </template>

    <ImageViewer v-model:open="imageViewerOpen" :items="imageViewerItems" />
  </div>
</template>
