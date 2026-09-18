<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { CircleAlert, Copy, Dices, Download, Plus, Trash2 } from '@lucide/vue'
import {
  createTtsMap,
  createTtsModuleUpdate,
  deleteTtsMap,
  deleteTtsMapPicture,
  deleteTtsModuleUpdate,
  fetchPrefs,
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
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { useAppSidePanel } from '@/composables/useAppSidePanel'
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

const route = useRoute()
const router = useRouter()
const { canEditContent } = useAdminEditMode()
const { isAuthenticated } = useAuth()
const { setCustomSide } = useAppSidePanel()

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

function onMapLinkClick(slug: string, event: MouseEvent) {
  if (
    event.button !== 0 ||
    event.metaKey ||
    event.ctrlKey ||
    event.shiftKey ||
    event.altKey
  ) {
    return
  }
  persistMapSlug(slug)
}

function onUpdatesClick(event: MouseEvent) {
  if (
    event.button !== 0 ||
    event.metaKey ||
    event.ctrlKey ||
    event.shiftKey ||
    event.altKey
  ) {
    return
  }
  persistMapSlug(null)
}

function drawMap() {
  if (!maps.value.length) return
  const pick = maps.value[Math.floor(Math.random() * maps.value.length)]!
  selectMap(pick.slug)
}

function scrollActiveIntoView() {
  const run = () => {
    document.querySelectorAll('.scenario-side-item--active').forEach((el) => {
      ;(el as HTMLElement).scrollIntoView({
        block: 'center',
        inline: 'nearest',
      })
    })
  }
  void nextTick(() => {
    run()
    requestAnimationFrame(() => {
      run()
      requestAnimationFrame(run)
    })
  })
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
      map.id === current.id
        ? {
            ...map,
            picture_count: current.pictures.length,
            updated_at: current.updated_at,
          }
        : map,
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
      map.id === detail.value?.id
        ? {
            ...map,
            picture_count: detail.value.pictures.length,
            updated_at: detail.value.updated_at,
          }
        : map,
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
    const [prefs] = await Promise.all([
      fetchPrefs().catch(() => null),
      loadLists(),
    ])
    const queryMap = Array.isArray(route.query.map)
      ? route.query.map[0]
      : route.query.map
    if (typeof queryMap === 'string') {
      persistMapSlug(queryMap)
    } else if (
      prefs?.tts_map_slug &&
      maps.value.some((map) => map.slug === prefs.tts_map_slug)
    ) {
      router.replace(mapTo(prefs.tts_map_slug))
    }
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
  [loading, apiOnline],
  ([isLoading, online]) => {
    setCustomSide(!isLoading && online)
  },
  { immediate: true },
)

watch(
  selectedSlug,
  (slug) => {
    if (!slug) {
      detail.value = null
      document.title = pageTitle('Module TTS')
      return
    }
    const summary = maps.value.find((map) => map.slug === slug)
    if (!summary) return
    document.title = pageTitle(summary.name)
    void loadDetail(summary.id)
  },
  { immediate: true },
)

watch(
  selectedSlug,
  () => {
    reportOpen.value = false
  },
)

watch(
  selectedSlug,
  () => scrollActiveIntoView(),
  { flush: 'post' },
)
</script>

<template>
  <div class="page-panel-scroll flex min-h-0 flex-1 flex-col">
    <p v-if="!apiOnline" class="shrink-0 text-sm text-muted-foreground">
      Impossible de charger le module TTS.
    </p>
    <p v-else-if="loading" class="shrink-0 text-sm text-muted-foreground">
      Chargement…
    </p>

    <template v-else>
      <Teleport defer to="#app-side-panel">
        <Card class="neon-panel flex h-full max-h-full min-h-0 flex-col overflow-hidden">
          <CardHeader class="shrink-0 pb-3">
            <CardTitle>Module TTS</CardTitle>
            <CardDescription>Tabletop Simulator</CardDescription>
          </CardHeader>
          <div class="scenario-side-list shrink-0 space-y-2 px-3 pb-3">
            <RouterLink
              :to="updatesTo()"
              class="scenario-side-item"
              :class="{ 'scenario-side-item--active': !selectedSlug }"
              :aria-current="!selectedSlug ? 'page' : undefined"
              @click="onUpdatesClick($event)"
            >
              Mises à jour
            </RouterLink>
            <button
              type="button"
              class="scenario-side-draw"
              :disabled="maps.length === 0"
              @click="drawMap"
            >
              <Dices class="size-4" />
              Tirer au sort
            </button>
          </div>
          <CardContent class="min-h-0 flex-1 overflow-y-auto pt-0">
            <nav class="scenario-side-list" aria-label="Maps du module TTS">
              <RouterLink
                v-for="map in maps"
                :key="map.id"
                :to="mapTo(map.slug)"
                class="scenario-side-item"
                :class="{
                  'scenario-side-item--active': selectedSlug === map.slug,
                }"
                :aria-current="selectedSlug === map.slug ? 'page' : undefined"
                @click="onMapLinkClick(map.slug, $event)"
              >
                {{ map.name }}
              </RouterLink>
            </nav>
            <form
              v-if="canEditContent"
              class="mt-4 space-y-2 border-t border-border/60 pt-3"
              @submit.prevent="createMap"
            >
              <Input
                v-model="newMapName"
                placeholder="Nom de la map"
                autocomplete="off"
              />
              <Button
                type="submit"
                size="sm"
                class="w-full"
                :disabled="creatingMap"
              >
                <Plus class="size-4" />
                {{ creatingMap ? 'Création…' : 'Ajouter une map' }}
              </Button>
            </form>
          </CardContent>
        </Card>
      </Teleport>

      <Card class="neon-panel mb-4 lg:hidden">
        <CardHeader class="pb-3">
          <CardTitle>Module TTS</CardTitle>
        </CardHeader>
        <div class="scenario-side-list shrink-0 space-y-2 px-3 pb-3">
          <RouterLink
            :to="updatesTo()"
            class="scenario-side-item"
            :class="{ 'scenario-side-item--active': !selectedSlug }"
            :aria-current="!selectedSlug ? 'page' : undefined"
            @click="onUpdatesClick($event)"
          >
            Mises à jour
          </RouterLink>
          <button
            type="button"
            class="scenario-side-draw"
            :disabled="maps.length === 0"
            @click="drawMap"
          >
            <Dices class="size-4" />
            Tirer au sort
          </button>
        </div>
        <CardContent class="pt-0">
          <nav class="scenario-side-list" aria-label="Maps du module TTS">
            <RouterLink
              v-for="map in maps"
              :key="`mobile-${map.id}`"
              :to="mapTo(map.slug)"
              class="scenario-side-item"
              :class="{
                'scenario-side-item--active': selectedSlug === map.slug,
              }"
              :aria-current="selectedSlug === map.slug ? 'page' : undefined"
              @click="onMapLinkClick(map.slug, $event)"
            >
              {{ map.name }}
            </RouterLink>
          </nav>
          <form
            v-if="canEditContent"
            class="mt-4 space-y-2 border-t border-border/60 pt-3"
            @submit.prevent="createMap"
          >
            <Input
              v-model="newMapName"
              placeholder="Nom de la map"
              autocomplete="off"
            />
            <Button
              type="submit"
              size="sm"
              class="w-full"
              :disabled="creatingMap"
            >
              <Plus class="size-4" />
              {{ creatingMap ? 'Création…' : 'Ajouter une map' }}
            </Button>
          </form>
        </CardContent>
      </Card>

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

      <div v-else class="grid gap-3 pb-4">
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
