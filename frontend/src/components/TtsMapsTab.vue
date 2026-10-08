<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { onKeyStroke, useScrollLock } from '@vueuse/core'
import { CircleAlert, Copy, Dices, Download, Plus, Trash2, X } from '@lucide/vue'
import {
  createMapUpdate,
  createTtsMap,
  deleteTtsMap,
  deleteTtsMapPicture,
  fetchTtsMap,
  fetchTtsMaps,
  renameTtsMap,
  updatePrefs,
  uploadTtsMapJson,
  uploadTtsMapPicture,
} from '@/lib/api'
import { withBase } from '@/lib/basePath'
import { pageTitle } from '@/lib/pageTitle'
import { copyTextToClipboard } from '@/lib/utils'
import type {
  TtsMapDetail,
  TtsMapPicture,
  TtsMapSummary,
} from '@/types/elo'
import ImageViewer, {
  type ImageViewerItem,
} from '@/components/ImageViewer.vue'
import TtsMapReportDialog from '@/components/TtsMapReportDialog.vue'
import TtsMapTile from '@/components/TtsMapTile.vue'
import TtsMapVariantsBlock from '@/components/TtsMapVariantsBlock.vue'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { useAuth } from '@/composables/useAuth'
import { useVirtualGrid } from '@/composables/useVirtualGrid'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

const emit = defineEmits<{
  mapChange: [payload: { id: number; slug: string; name: string } | null]
  updatesChange: []
}>()

const route = useRoute()
const router = useRouter()
const { canEditContent } = useAdminEditMode()
const { isAuthenticated } = useAuth()

const maps = ref<TtsMapSummary[]>([])

function sortMaps(items: TtsMapSummary[]): TtsMapSummary[] {
  return [...items].sort((a, b) =>
    a.name.localeCompare(b.name, 'fr', { sensitivity: 'base' }),
  )
}
const detail = ref<TtsMapDetail | null>(null)
const loading = ref(true)
const detailLoading = ref(false)
const apiOnline = ref(true)
const newMapName = ref('')
const creatingMap = ref(false)
const renaming = ref(false)
const renameDraft = ref('')
const uploadingPictures = ref(false)
const picturesInput = ref<HTMLInputElement | null>(null)
const updateOpen = ref(false)
const updateDraft = ref('')
const updateJsonFile = ref<File | null>(null)
const updateJsonInput = ref<HTMLInputElement | null>(null)
const publishingUpdate = ref(false)
const imageViewerOpen = ref(false)
const imageViewerIndex = ref(0)
const reportOpen = ref(false)
const scrollEl = ref<HTMLElement | null>(null)
const gridEl = ref<HTMLElement | null>(null)

const canPublishUpdate = computed(
  () => updateDraft.value.trim().length > 0 && !publishingUpdate.value,
)

const pageBody = typeof document !== 'undefined' ? document.body : null
const updateScrollLock = useScrollLock(pageBody)
watch(updateOpen, (value) => {
  updateScrollLock.value = value
  if (!value) {
    updateDraft.value = ''
    updateJsonFile.value = null
    publishingUpdate.value = false
    if (updateJsonInput.value) updateJsonInput.value.value = ''
  }
})

onKeyStroke('Escape', (event) => {
  if (!updateOpen.value) return
  event.preventDefault()
  updateOpen.value = false
})

onBeforeUnmount(() => {
  updateScrollLock.value = false
})

const { visibleItems, paddingTop, paddingBottom } = useVirtualGrid({
  items: maps,
  scrollEl,
  gridEl,
})

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
  return { name: 'maps' as const, query: {} }
}

function mapTo(slug: string) {
  return { name: 'maps' as const, query: { map: slug } }
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

async function loadLists() {
  maps.value = sortMaps(await fetchTtsMaps())
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
    maps.value = sortMaps([...maps.value, summaryFromDetail(created)])
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
    maps.value = sortMaps(
      maps.value.map((map) =>
        map.id === detail.value?.id
          ? { ...map, name: detail.value.name, updated_at: detail.value.updated_at }
          : map,
      ),
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
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible de supprimer la map',
    )
  }
}

function onUpdateJsonPicked(event: Event) {
  const input = event.target as HTMLInputElement
  updateJsonFile.value = input.files?.[0] ?? null
}

function onUpdateBackdropClick(event: MouseEvent) {
  if (event.target === event.currentTarget) updateOpen.value = false
}

async function publishUpdate() {
  if (!detail.value) return
  const description = updateDraft.value.trim()
  if (!description) {
    toast.error('La description est requise')
    return
  }
  const file = updateJsonFile.value
  publishingUpdate.value = true
  try {
    if (file) {
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
    }
    await createMapUpdate(detail.value.id, description)
    emit('updatesChange')
    updateOpen.value = false
    toast.success('Mise à jour publiée')
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de publier la mise à jour',
    )
  } finally {
    publishingUpdate.value = false
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
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible de supprimer la photo',
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
        : 'Impossible de charger Map TTS',
    )
  } finally {
    loading.value = false
  }
})

watch(
  selectedSlug,
  (slug) => {
    if (!loading.value) persistMapSlug(slug)
    if (!slug) {
      detail.value = null
      emit('mapChange', null)
      document.title = pageTitle('Map TTS')
      return
    }
    const summary = maps.value.find((map) => map.slug === slug)
    if (!summary) return
    emit('mapChange', {
      id: summary.id,
      slug: summary.slug,
      name: summary.name,
    })
    document.title = pageTitle(summary.name)
    void loadDetail(summary.id)
  },
  { immediate: true },
)

watch(
  () => detail.value?.name,
  (name) => {
    if (!detail.value || !selectedSlug.value || !name) return
    emit('mapChange', {
      id: detail.value.id,
      slug: selectedSlug.value,
      name,
    })
    document.title = pageTitle(name)
  },
)

watch(selectedSlug, () => {
  reportOpen.value = false
  updateOpen.value = false
})
</script>

<template>
  <div ref="scrollEl" class="page-panel-scroll min-h-0 flex-1">
    <p v-if="!apiOnline" class="shrink-0 text-sm text-muted-foreground">
      Impossible de charger Map TTS.
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
            <Button
              v-if="canEditContent"
              type="button"
              variant="outline"
              size="sm"
              @click="updateOpen = true"
            >
              Ajouter une mise à jour
            </Button>
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
                  loading="lazy"
                  decoding="async"
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
        <div
          v-else
          ref="gridEl"
          class="tts-map-grid"
          :style="{
            paddingTop: `${paddingTop}px`,
            paddingBottom: `${paddingBottom}px`,
          }"
        >
          <TtsMapTile v-for="map in visibleItems" :key="map.id" :map="map" />
        </div>
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
    <Teleport to="body">
      <div
        v-if="updateOpen && detail"
        class="player-detail-overlay"
        role="dialog"
        aria-modal="true"
        aria-labelledby="map-update-title"
        @click="onUpdateBackdropClick"
      >
        <Card class="player-detail-modal neon-panel w-full max-w-lg">
          <CardHeader class="relative pr-12">
            <CardTitle id="map-update-title">Ajouter une mise à jour</CardTitle>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              class="absolute top-3 right-3"
              aria-label="Fermer"
              @click="updateOpen = false"
            >
              <X class="size-4" />
            </Button>
          </CardHeader>
          <CardContent>
            <form class="space-y-4" @submit.prevent="publishUpdate">
              <div class="grid gap-2">
                <Label for="map-update-description">Description</Label>
                <textarea
                  id="map-update-description"
                  v-model="updateDraft"
                  rows="4"
                  maxlength="4000"
                  class="flex w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-xs outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                  placeholder="Décrivez la mise à jour…"
                />
              </div>
              <div class="grid gap-2">
                <Label for="map-update-json">JSON (optionnel)</Label>
                <input
                  id="map-update-json"
                  ref="updateJsonInput"
                  type="file"
                  accept=".json,application/json"
                  class="text-sm text-muted-foreground file:mr-3 file:rounded-md file:border-0 file:bg-primary/15 file:px-3 file:py-1.5 file:text-sm file:font-medium file:text-primary"
                  @change="onUpdateJsonPicked"
                />
              </div>
              <div class="flex justify-end">
                <Button type="submit" size="sm" :disabled="!canPublishUpdate">
                  {{ publishingUpdate ? 'Publication…' : 'Publier' }}
                </Button>
              </div>
            </form>
          </CardContent>
        </Card>
      </div>
    </Teleport>
  </div>
</template>
