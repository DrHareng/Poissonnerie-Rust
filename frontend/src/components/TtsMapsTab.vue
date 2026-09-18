<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { CircleAlert, Copy, Dices, Download, Plus, Trash2 } from '@lucide/vue'
import {
  createTtsMap,
  createTtsModuleUpdate,
  deleteTtsMap,
  deleteTtsMapPicture,
  deleteTtsModuleUpdate,
  fetchTtsContentImages,
  fetchTtsMap,
  fetchTtsMaps,
  fetchTtsModuleUpdates,
  renameTtsMap,
  updatePrefs,
  updateTtsModuleUpdate,
  uploadTtsMapJson,
  uploadTtsMapPicture,
} from '@/lib/api'
import { withBase } from '@/lib/basePath'
import { pageTitle } from '@/lib/pageTitle'
import { copyTextToClipboard } from '@/lib/utils'
import type {
  TtsContentImage,
  TtsMapDetail,
  TtsMapPicture,
  TtsMapSummary,
  TtsModuleUpdate,
} from '@/types/elo'
import AdminContentEditor from '@/components/AdminContentEditor.vue'
import ImageViewer, {
  type ImageViewerItem,
} from '@/components/ImageViewer.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import MarkdownEditor from '@/components/MarkdownEditor.vue'
import TtsMapReportDialog from '@/components/TtsMapReportDialog.vue'
import TtsMapTile from '@/components/TtsMapTile.vue'
import TtsMapVariantsBlock from '@/components/TtsMapVariantsBlock.vue'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { useAuth } from '@/composables/useAuth'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Input } from '@/components/ui/input'

const emit = defineEmits<{
  mapChange: [payload: { slug: string; name: string } | null]
}>()

const route = useRoute()
const router = useRouter()
const { canEditContent } = useAdminEditMode()
const { isAuthenticated } = useAuth()

const maps = ref<TtsMapSummary[]>([])
const updates = ref<TtsModuleUpdate[]>([])
const detail = ref<TtsMapDetail | null>(null)
const contentImages = ref<TtsContentImage[]>([])
const loading = ref(true)
const detailLoading = ref(false)
const apiOnline = ref(true)
const newMapName = ref('')
const creatingMap = ref(false)
const newUpdateBody = ref('')
const creatingUpdate = ref(false)
const renaming = ref(false)
const renameDraft = ref('')
const uploadingJson = ref(false)
const uploadingPictures = ref(false)
const jsonInput = ref<HTMLInputElement | null>(null)
const picturesInput = ref<HTMLInputElement | null>(null)
const imageViewerOpen = ref(false)
const imageViewerIndex = ref(0)
const reportOpen = ref(false)

const extraImages = computed(() =>
  contentImages.value.map((image) => ({
    label: image.label,
    value: image.path,
  })),
)

const selectedSlug = computed(() => {
  const raw = route.query.map
  const value = Array.isArray(raw) ? raw[0] : raw
  if (typeof value === 'string' && maps.value.some((map) => map.slug === value)) {
    return value
  }
  return null
})

const viewerItems = computed((): ImageViewerItem[] => {
  if (!detail.value) return []
  return detail.value.pictures.map((picture) => ({
    src: withBase(picture.url),
    alt: picture.original_name || picture.filename,
    caption: picture.original_name || picture.filename,
  }))
})

function updatesTo() {
  return { name: 'ressources' as const, query: {} }
}

function mapTo(slug: string) {
  return { name: 'ressources' as const, query: { map: slug } }
}

function persistMapSlug(slug: string | null) {
  void updatePrefs({ tts_map_slug: slug ?? '' }).catch(() => {
    // Keep the local choice even if persistence fails.
  })
}

function selectMap(slug: string | null) {
  persistMapSlug(slug)
  router.replace(slug ? mapTo(slug) : updatesTo())
}

function drawMap() {
  if (!maps.value.length) return
  const pick = maps.value[Math.floor(Math.random() * maps.value.length)]!
  selectMap(pick.slug)
}

function formatUpdateDate(timestamp: number) {
  return new Intl.DateTimeFormat('fr-FR', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(timestamp * 1000))
}

function openPicture(index: number) {
  if (!viewerItems.value.length) return
  imageViewerIndex.value = index
  imageViewerOpen.value = true
}

async function copyPictureToken(picture: TtsMapPicture) {
  const token = `[img]${picture.url}[img]`
  try {
    await copyTextToClipboard(token)
    toast.success('Lien markdown copié')
  } catch {
    toast.error('Impossible de copier le lien')
  }
}

async function loadContentImages() {
  if (!canEditContent.value) return
  try {
    contentImages.value = await fetchTtsContentImages()
  } catch {
    contentImages.value = []
  }
}

async function loadLists() {
  const [mapList, updateList] = await Promise.all([
    fetchTtsMaps(),
    fetchTtsModuleUpdates(),
  ])
  maps.value = mapList
  updates.value = updateList
}

async function loadDetail(id: number) {
  detailLoading.value = true
  try {
    detail.value = await fetchTtsMap(id)
    renameDraft.value = detail.value.name
  } catch (error) {
    detail.value = null
    toast.error(
      error instanceof Error ? error.message : 'Impossible de charger la map',
    )
  } finally {
    detailLoading.value = false
  }
}

function summaryFromDetail(map: TtsMapDetail): TtsMapSummary {
  return {
    id: map.id,
    slug: map.slug,
    name: map.name,
    has_json: Boolean(map.json_filename),
    json_filename: map.json_filename,
    picture_count: map.pictures.length,
    pictures: map.pictures,
    created_at: map.created_at,
    updated_at: map.updated_at,
  }
}

async function createMap() {
  const name = newMapName.value.trim()
  if (!name) {
    toast.error('Le nom est requis')
    return
  }
  creatingMap.value = true
  try {
    const created = await createTtsMap(name)
    newMapName.value = ''
    maps.value = [...maps.value, summaryFromDetail(created)]
    toast.success('Map créée')
    selectMap(created.slug)
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible de créer la map',
    )
  } finally {
    creatingMap.value = false
  }
}

async function saveRename() {
  if (!detail.value) return
  const name = renameDraft.value.trim()
  if (!name) {
    toast.error('Le nom est requis')
    return
  }
  renaming.value = true
  try {
    detail.value = await renameTtsMap(detail.value.id, name)
    maps.value = maps.value.map((map) =>
      map.id === detail.value?.id
        ? { ...map, name: detail.value.name, updated_at: detail.value.updated_at }
        : map,
    )
    toast.success('Map enregistrée')
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible de renommer la map',
    )
  } finally {
    renaming.value = false
  }
}

async function removeMap() {
  if (!detail.value) return
  if (!window.confirm(`Supprimer la map « ${detail.value.name} » ?`)) return
  const id = detail.value.id
  try {
    await deleteTtsMap(id)
    maps.value = maps.value.filter((map) => map.id !== id)
    toast.success('Map supprimée')
    selectMap(null)
    detail.value = null
    await loadContentImages()
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible de supprimer la map',
    )
  }
}

async function onJsonSelected(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file || !detail.value) return
  uploadingJson.value = true
  try {
    detail.value = await uploadTtsMapJson(detail.value.id, file)
    maps.value = maps.value.map((map) =>
      map.id === detail.value?.id
        ? {
            ...map,
            has_json: true,
            json_filename: detail.value.json_filename,
            updated_at: detail.value.updated_at,
          }
        : map,
    )
    toast.success('JSON enregistré')
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible d’envoyer le JSON',
    )
  } finally {
    uploadingJson.value = false
  }
}

async function onPicturesSelected(event: Event) {
  const input = event.target as HTMLInputElement
  const files = [...(input.files ?? [])]
  input.value = ''
  if (!files.length || !detail.value) return
  uploadingPictures.value = true
  try {
    let current = detail.value
    for (const file of files) {
      current = await uploadTtsMapPicture(current.id, file)
    }
    detail.value = current
    maps.value = maps.value.map((map) =>
      map.id === current.id ? summaryFromDetail(current) : map,
    )
    toast.success(
      files.length > 1 ? 'Photos enregistrées' : 'Photo enregistrée',
    )
    await loadContentImages()
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible d’envoyer les photos',
    )
  } finally {
    uploadingPictures.value = false
  }
}

async function removePicture(picture: TtsMapPicture) {
  if (!detail.value) return
  if (!window.confirm(`Supprimer la photo « ${picture.original_name} » ?`)) {
    return
  }
  try {
    detail.value = await deleteTtsMapPicture(detail.value.id, picture.filename)
    maps.value = maps.value.map((map) =>
      map.id === detail.value?.id ? summaryFromDetail(detail.value) : map,
    )
    toast.success('Photo supprimée')
    await loadContentImages()
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible de supprimer la photo',
    )
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

onMounted(async () => {
  try {
    await loadLists()
    persistMapSlug(selectedSlug.value)
    apiOnline.value = true
  } catch (error) {
    apiOnline.value = false
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de charger le module TTS',
    )
  } finally {
    loading.value = false
  }
  await loadContentImages()
})

watch(canEditContent, (canEdit) => {
  if (canEdit) void loadContentImages()
})

watch(
  selectedSlug,
  (slug) => {
    if (!loading.value) persistMapSlug(slug)
    if (!slug) {
      detail.value = null
      emit('mapChange', null)
      document.title = pageTitle('Module TTS')
      return
    }
    const summary = maps.value.find((map) => map.slug === slug)
    if (!summary) return
    emit('mapChange', { slug: summary.slug, name: summary.name })
    document.title = pageTitle(summary.name)
    void loadDetail(summary.id)
  },
  { immediate: true },
)

watch(
  () => detail.value?.name,
  (name) => {
    if (!selectedSlug.value || !name) return
    emit('mapChange', { slug: selectedSlug.value, name })
    document.title = pageTitle(name)
  },
)

watch(selectedSlug, () => {
  reportOpen.value = false
})
</script>

<template>
  <div class="page-panel-scroll min-h-0 flex-1">
    <p v-if="!apiOnline" class="shrink-0 text-sm text-muted-foreground">
      Impossible de charger le module TTS.
    </p>
    <p v-else-if="loading" class="shrink-0 text-sm text-muted-foreground">
      Chargement…
    </p>

    <template v-else>
      <template v-if="selectedSlug">
        <p v-if="detailLoading" class="text-sm text-muted-foreground">
          Chargement…
        </p>
        <template v-else-if="detail">
          <section class="page-header relative shrink-0">
            <h2 class="page-title text-2xl">{{ detail.name }}</h2>
            <form
              v-if="canEditContent"
              class="mt-3 flex flex-wrap items-end gap-2"
              @submit.prevent="saveRename"
            >
              <Input
                v-model="renameDraft"
                class="max-w-sm"
                placeholder="Nom"
                autocomplete="off"
              />
              <Button type="submit" size="sm" :disabled="renaming">
                {{ renaming ? 'Enregistrement…' : 'Renommer' }}
              </Button>
            </form>
          </section>

          <div class="flex flex-wrap items-center gap-2 pb-4">
            <Button
              v-if="detail.json_url"
              as="a"
              :href="withBase(detail.json_url)"
              size="sm"
            >
              <Download class="size-4" />
              Télécharger le JSON
            </Button>
            <span
              v-if="detail.json_url && detail.json_filename"
              class="min-w-0 truncate font-mono text-sm text-muted-foreground"
              :title="detail.json_filename"
            >
              {{ detail.json_filename }}
            </span>
            <p v-else-if="!detail.json_url" class="text-sm text-muted-foreground">
              Aucun JSON pour cette map.
            </p>
            <Button
              v-if="isAuthenticated"
              type="button"
              variant="outline"
              size="sm"
              @click="reportOpen = true"
            >
              <CircleAlert class="size-4" />
              Remonter un soucis
            </Button>
            <template v-if="canEditContent">
              <input
                ref="jsonInput"
                type="file"
                accept=".json,application/json"
                class="sr-only"
                @change="onJsonSelected"
              />
              <Button
                type="button"
                variant="outline"
                size="sm"
                :disabled="uploadingJson"
                @click="jsonInput?.click()"
              >
                {{ uploadingJson ? 'Envoi…' : 'Uploader un JSON' }}
              </Button>
            </template>
          </div>

          <div v-if="detail.pictures.length" class="grid gap-3 pb-4 sm:grid-cols-2">
            <figure
              v-for="(picture, index) in detail.pictures"
              :key="picture.id"
              class="neon-panel overflow-hidden"
            >
              <button
                type="button"
                class="block w-full"
                @click="openPicture(index)"
              >
                <img
                  :src="withBase(picture.url)"
                  :alt="picture.original_name"
                  class="max-h-80 w-full object-contain"
                />
              </button>
              <figcaption
                v-if="canEditContent"
                class="flex flex-wrap items-center justify-between gap-2 px-3 py-2 text-xs text-muted-foreground"
              >
                <span class="min-w-0 truncate">{{ picture.original_name }}</span>
                <span class="flex gap-1">
                  <Button
                    type="button"
                    variant="ghost"
                    size="xs"
                    title="Copier le lien markdown"
                    @click="copyPictureToken(picture)"
                  >
                    <Copy class="size-3.5" />
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="xs"
                    title="Supprimer la photo"
                    @click="removePicture(picture)"
                  >
                    <Trash2 class="size-3.5" />
                  </Button>
                </span>
              </figcaption>
            </figure>
          </div>
          <p v-else class="pb-4 text-sm text-muted-foreground">
            Aucune photo pour cette map.
          </p>

          <TtsMapVariantsBlock class="mb-4 shrink-0" :map-id="detail.id" />

          <div v-if="canEditContent" class="flex flex-wrap items-center gap-2 pb-6">
            <input
              ref="picturesInput"
              type="file"
              accept="image/png,image/jpeg,image/webp,image/gif"
              multiple
              class="sr-only"
              @change="onPicturesSelected"
            />
            <Button
              type="button"
              variant="outline"
              size="sm"
              :disabled="uploadingPictures"
              @click="picturesInput?.click()"
            >
              {{ uploadingPictures ? 'Envoi…' : 'Ajouter des photos' }}
            </Button>
            <Button
              type="button"
              variant="destructive"
              size="sm"
              @click="removeMap"
            >
              <Trash2 class="size-4" />
              Supprimer la map
            </Button>
          </div>
        </template>
        <p v-else class="text-sm text-muted-foreground">
          Map introuvable.
        </p>
      </template>

      <div v-else class="grid gap-6 pb-4">
        <div class="flex flex-wrap items-end gap-2">
          <Button
            type="button"
            size="sm"
            :disabled="maps.length === 0"
            @click="drawMap"
          >
            <Dices class="size-4" />
            Tirer au sort
          </Button>
          <form
            v-if="canEditContent"
            class="flex min-w-0 flex-1 flex-wrap items-end gap-2"
            @submit.prevent="createMap"
          >
            <Input
              v-model="newMapName"
              class="max-w-xs"
              placeholder="Nom de la map"
              autocomplete="off"
            />
            <Button type="submit" size="sm" :disabled="creatingMap">
              <Plus class="size-4" />
              {{ creatingMap ? 'Création…' : 'Ajouter une map' }}
            </Button>
          </form>
        </div>

        <p v-if="maps.length === 0" class="text-sm text-muted-foreground">
          Aucune map pour l’instant.
        </p>
        <div v-else class="tts-map-grid">
          <TtsMapTile v-for="map in maps" :key="map.id" :map="map" />
        </div>

        <section class="grid gap-3">
          <h2 class="page-title text-xl">Mises à jour</h2>
          <Card v-if="canEditContent" class="neon-panel shrink-0">
            <CardHeader>
              <CardTitle>Nouvelle mise à jour</CardTitle>
            </CardHeader>
            <CardContent class="space-y-3">
              <MarkdownEditor
                v-model="newUpdateBody"
                :rows="8"
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
            <CardHeader :class="{ 'pr-28': canEditContent }">
              <CardTitle class="text-base font-medium">
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
                :rows="8"
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
                class="absolute top-3 right-20 z-10"
                @click="removeUpdate(update)"
              >
                <Trash2 class="size-3.5" />
                Supprimer
              </Button>
            </CardContent>
          </Card>
        </section>
      </div>
    </template>

    <ImageViewer
      v-model:open="imageViewerOpen"
      v-model:index="imageViewerIndex"
      :items="viewerItems"
    />
    <TtsMapReportDialog
      v-if="detail"
      v-model:open="reportOpen"
      :map-id="detail.id"
      :map-name="detail.name"
    />
  </div>
</template>
