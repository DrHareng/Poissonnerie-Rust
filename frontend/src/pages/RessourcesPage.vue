<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { BookOpen } from '@lucide/vue'
import { toast } from 'vue-sonner'
import { fetchRessources, updateRessources } from '@/lib/api'
import AdminContentEditor from '@/components/AdminContentEditor.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

const { canEditContent } = useAdminEditMode()

const bodyMd = ref('')
const loading = ref(true)
const apiOnline = ref(true)

async function load() {
  loading.value = true
  try {
    const content = await fetchRessources()
    bodyMd.value = content.body_md
    apiOnline.value = true
  } catch (error) {
    apiOnline.value = false
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de charger les ressources',
    )
  } finally {
    loading.value = false
  }
}

async function save(payload: { body: string }) {
  const content = await updateRessources({ body_md: payload.body })
  bodyMd.value = content.body_md
  toast.success('Ressources enregistrées')
}

onMounted(load)
</script>

<template>
  <div class="page-stack">
    <nav class="page-title-tabs" aria-label="Ressources">
      <div class="page-title-tabs-list">
        <h1 class="page-title-tab page-title-tab--active" aria-current="page">
          Ressources
        </h1>
      </div>
    </nav>

    <Alert v-if="!apiOnline" variant="destructive" class="neon-panel-accent shrink-0">
      <AlertTitle>API indisponible</AlertTitle>
      <AlertDescription>
        Lancez le serveur Rust puis rechargez la page.
      </AlertDescription>
    </Alert>

    <p v-else-if="loading" class="shrink-0 text-sm text-muted-foreground">
      Chargement…
    </p>

    <Card v-else class="neon-panel page-panel-scroll">
      <CardHeader :class="{ 'pr-24': canEditContent }">
        <CardTitle class="flex items-center gap-2">
          <BookOpen class="size-5 text-primary" />
          Liens et ressources
        </CardTitle>
      </CardHeader>
      <CardContent>
        <AdminContentEditor
          :can-edit="canEditContent"
          :body="bodyMd"
          :rows="24"
          simple-markdown
          :persist="save"
        >
          <MarkdownContent v-if="bodyMd.trim()" :source="bodyMd" />
          <p v-else class="text-sm text-muted-foreground">
            Aucune ressource pour l’instant.
          </p>
        </AdminContentEditor>
      </CardContent>
    </Card>
  </div>
</template>
