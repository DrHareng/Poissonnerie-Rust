<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { Trash2 } from '@lucide/vue'
import {
  createTtsModuleUpdate,
  deleteTtsModuleUpdate,
  fetchTtsContentImages,
  fetchTtsModuleUpdates,
  updateTtsModuleUpdate,
} from '@/lib/api'
import type { TtsContentImage, TtsModuleUpdate } from '@/types/elo'
import AdminContentEditor from '@/components/AdminContentEditor.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import MarkdownEditor from '@/components/MarkdownEditor.vue'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { useAppSidePanel } from '@/composables/useAppSidePanel'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

const props = withDefaults(
  defineProps<{
    /** Affiche aussi le bloc sous le header (écrans étroits). */
    mobile?: boolean
  }>(),
  { mobile: true },
)
const { canEditContent } = useAdminEditMode()
const { setCustomSide } = useAppSidePanel()

const updates = ref<TtsModuleUpdate[]>([])
const contentImages = ref<TtsContentImage[]>([])
const newUpdateBody = ref('')
const creatingUpdate = ref(false)
const loaded = ref(false)

const extraImages = computed(() =>
  contentImages.value.map((image) => ({
    label: image.label,
    value: image.path,
  })),
)

const showPanel = computed(
  () => loaded.value && (updates.value.length > 0 || canEditContent.value),
)

function formatUpdateDate(timestamp: number) {
  return new Intl.DateTimeFormat('fr-FR', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(timestamp * 1000))
}

async function loadContentImages() {
  if (!canEditContent.value) return
  try {
    contentImages.value = await fetchTtsContentImages()
  } catch {
    contentImages.value = []
  }
}

async function loadUpdates() {
  try {
    updates.value = await fetchTtsModuleUpdates()
  } catch (error) {
    updates.value = []
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de charger les mises à jour',
    )
  } finally {
    loaded.value = true
  }
}

async function createUpdate() {
  const body = newUpdateBody.value.trim()
  if (!body) {
    toast.error('La description est requise')
    return
  }
  creatingUpdate.value = true
  try {
    const created = await createTtsModuleUpdate(body)
    updates.value = [created, ...updates.value]
    newUpdateBody.value = ''
    toast.success('Mise à jour publiée')
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de publier la mise à jour',
    )
  } finally {
    creatingUpdate.value = false
  }
}

async function saveUpdate(id: number, payload: { body: string }) {
  const updated = await updateTtsModuleUpdate(id, payload.body)
  updates.value = updates.value.map((item) =>
    item.id === id ? updated : item,
  )
  toast.success('Mise à jour enregistrée')
}

async function removeUpdate(update: TtsModuleUpdate) {
  if (!window.confirm('Supprimer cette mise à jour ?')) return
  try {
    await deleteTtsModuleUpdate(update.id)
    updates.value = updates.value.filter((item) => item.id !== update.id)
    toast.success('Mise à jour supprimée')
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de supprimer la mise à jour',
    )
  }
}

watch(
  showPanel,
  (active) => setCustomSide(active),
  { immediate: true },
)

watch(canEditContent, (canEdit) => {
  if (canEdit) void loadContentImages()
})

onMounted(async () => {
  await loadUpdates()
  await loadContentImages()
})
</script>

<template>
  <Teleport defer to="#app-side-panel">
    <div
      v-if="showPanel"
      class="flex h-full min-h-0 flex-col gap-3 overflow-y-auto pr-1"
    >
      <h2 class="page-title shrink-0 text-lg">Mises à jour</h2>
      <Card v-if="canEditContent" class="neon-panel shrink-0">
        <CardHeader>
          <CardTitle class="text-base">Nouvelle mise à jour</CardTitle>
        </CardHeader>
        <CardContent class="space-y-3">
          <MarkdownEditor
            v-model="newUpdateBody"
            :rows="6"
            simple
            :extra-images="extraImages"
          />
          <div class="flex justify-end">
            <Button
              type="button"
              size="sm"
              :disabled="creatingUpdate"
              @click="createUpdate"
            >
              {{ creatingUpdate ? 'Publication…' : 'Publier' }}
            </Button>
          </div>
        </CardContent>
      </Card>

      <p v-if="updates.length === 0" class="text-sm text-muted-foreground">
        Aucune mise à jour pour l’instant.
      </p>

      <Card
        v-for="update in updates"
        :key="update.id"
        class="neon-panel relative shrink-0"
      >
        <CardHeader :class="{ 'pr-16': canEditContent }">
          <CardTitle class="text-sm font-medium">
            {{ formatUpdateDate(update.created_at) }}
          </CardTitle>
          <CardDescription v-if="update.updated_at !== update.created_at">
            Modifié le {{ formatUpdateDate(update.updated_at) }}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <AdminContentEditor
            :can-edit="canEditContent"
            :body="update.body_md"
            :rows="6"
            simple-markdown
            :extra-images="extraImages"
            :persist="(payload) => saveUpdate(update.id, payload)"
          >
            <MarkdownContent :source="update.body_md" />
          </AdminContentEditor>
          <Button
            v-if="canEditContent"
            type="button"
            variant="ghost"
            size="sm"
            class="absolute top-2 right-2 z-10"
            @click="removeUpdate(update)"
          >
            <Trash2 class="size-3.5" />
          </Button>
        </CardContent>
      </Card>
    </div>
  </Teleport>

  <div
    v-if="showPanel && mobile"
    class="mb-4 grid gap-3 lg:hidden"
  >
    <h2 class="page-title text-lg">Mises à jour</h2>
    <Card v-if="canEditContent" class="neon-panel shrink-0">
      <CardHeader>
        <CardTitle class="text-base">Nouvelle mise à jour</CardTitle>
      </CardHeader>
      <CardContent class="space-y-3">
        <MarkdownEditor
          v-model="newUpdateBody"
          :rows="6"
          simple
          :extra-images="extraImages"
        />
        <div class="flex justify-end">
          <Button
            type="button"
            size="sm"
            :disabled="creatingUpdate"
            @click="createUpdate"
          >
            {{ creatingUpdate ? 'Publication…' : 'Publier' }}
          </Button>
        </div>
      </CardContent>
    </Card>
    <p v-if="updates.length === 0" class="text-sm text-muted-foreground">
      Aucune mise à jour pour l’instant.
    </p>
    <Card
      v-for="update in updates"
      :key="`mobile-${update.id}`"
      class="neon-panel relative shrink-0"
    >
      <CardHeader>
        <CardTitle class="text-sm font-medium">
          {{ formatUpdateDate(update.created_at) }}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <MarkdownContent :source="update.body_md" />
      </CardContent>
    </Card>
  </div>
</template>
