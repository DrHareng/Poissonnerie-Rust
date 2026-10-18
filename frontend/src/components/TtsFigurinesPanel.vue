<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useAppSidePanel } from '@/composables/useAppSidePanel'
import {
  clearTtsFigurinePhoto,
  createTtsFigurine,
  deleteTtsFigurine,
  fetchTtsFigurines,
  fetchTtsProfileLabels,
  updateTtsFigurine,
  uploadTtsFigurinePhoto,
} from '@/lib/api'
import { withBase } from '@/lib/basePath'
import type { TtsFigurine, TtsFigurineWrite, TtsProfileLabel } from '@/types/elo'

type Draft = TtsFigurineWrite

const { setCustomSide } = useAppSidePanel()

const figurines = ref<TtsFigurine[]>([])
const labels = ref<TtsProfileLabel[]>([])
const search = ref('')
const loading = ref(true)
const loadError = ref('')
const saving = ref(false)
const panelOpen = ref(false)
const editingId = ref<number | null>(null)
const draft = ref<Draft>(emptyDraft())
const photoFile = ref<File | null>(null)
const photoPreview = ref('')
const existingPhoto = ref('')
const removeExistingPhoto = ref(false)
const iscFocused = ref(false)
const loadoutFocused = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)

const filtered = computed(() => {
  const needle = search.value.trim().toLowerCase()
  if (!needle) return figurines.value
  return figurines.value.filter((figurine) =>
    [figurine.isc, figurine.loadout, figurine.painter, figurine.asset_bundle_url]
      .join(' ')
      .toLowerCase()
      .includes(needle),
  )
})

const iscOptions = computed(() => uniqueMatches(labels.value.map((label) => label.isc), draft.value.isc))

const loadoutOptions = computed(() => {
  const isc = draft.value.isc.trim().toLowerCase()
  const exact = labels.value.filter((label) => label.isc.toLowerCase() === isc)
  if (exact.length) {
    return uniqueMatches(exact.map((label) => label.loadout), draft.value.loadout, true)
  }
  return uniqueMatches(labels.value.map((label) => label.loadout), draft.value.loadout)
})

const showIscOptions = computed(() => iscFocused.value && iscOptions.value.length > 0)
const showLoadoutOptions = computed(() => loadoutFocused.value && loadoutOptions.value.length > 0)

const previewSrc = computed(() => photoPreview.value || (!removeExistingPhoto.value ? existingPhoto.value : ''))

function emptyDraft(): Draft {
  return { isc: '', loadout: '', painter: '', asset_bundle_url: '' }
}

function uniqueMatches(values: string[], needle: string, listWhenEmpty = false) {
  const query = needle.trim().toLowerCase()
  if (!query && !listWhenEmpty) return []
  const seen = new Set<string>()
  const matches: string[] = []
  for (const value of values) {
    const trimmed = value.trim()
    if (!trimmed) continue
    const key = trimmed.toLowerCase()
    if (query && (!key.includes(query) || key === query)) continue
    if (seen.has(key)) continue
    seen.add(key)
    matches.push(trimmed)
    if (matches.length >= 8) break
  }
  return matches
}

function photoSrc(figurine: TtsFigurine) {
  if (!figurine.photo_url) return ''
  return withBase(`${figurine.photo_url}?thumb=1&v=${figurine.updated_at}`)
}

function fullPhotoSrc(figurine: TtsFigurine) {
  if (!figurine.photo_url) return ''
  return withBase(`${figurine.photo_url}?v=${figurine.updated_at}`)
}

function revokePreview() {
  if (photoPreview.value.startsWith('blob:')) URL.revokeObjectURL(photoPreview.value)
  photoPreview.value = ''
}

function setPhoto(file: File) {
  revokePreview()
  photoFile.value = file
  removeExistingPhoto.value = false
  photoPreview.value = URL.createObjectURL(file)
}

function onPaste(event: ClipboardEvent) {
  const items = event.clipboardData?.items
  if (!items) return
  for (const item of items) {
    if (!item.type.startsWith('image/')) continue
    const file = item.getAsFile()
    if (!file) continue
    event.preventDefault()
    setPhoto(file)
    return
  }
}

function onFileChange(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  if (file) setPhoto(file)
  input.value = ''
}

function onDrop(event: DragEvent) {
  const file = event.dataTransfer?.files?.[0]
  if (file?.type.startsWith('image/')) setPhoto(file)
}

function openCreate() {
  editingId.value = null
  draft.value = emptyDraft()
  existingPhoto.value = ''
  removeExistingPhoto.value = false
  photoFile.value = null
  revokePreview()
  panelOpen.value = true
}

function openEdit(figurine: TtsFigurine) {
  editingId.value = figurine.id
  draft.value = {
    isc: figurine.isc,
    loadout: figurine.loadout,
    painter: figurine.painter,
    asset_bundle_url: figurine.asset_bundle_url,
  }
  existingPhoto.value = fullPhotoSrc(figurine)
  removeExistingPhoto.value = false
  photoFile.value = null
  revokePreview()
  panelOpen.value = true
}

function closePanel() {
  panelOpen.value = false
  photoFile.value = null
  revokePreview()
}

function forgetPhoto() {
  photoFile.value = null
  revokePreview()
  if (existingPhoto.value) removeExistingPhoto.value = true
}

async function loadFigurines() {
  loading.value = true
  loadError.value = ''
  try {
    figurines.value = await fetchTtsFigurines()
  } catch (error) {
    loadError.value = error instanceof Error ? error.message : 'Impossible de charger les figurines'
  } finally {
    loading.value = false
  }
}

async function save() {
  saving.value = true
  const creating = editingId.value == null
  try {
    const payload = {
      isc: draft.value.isc,
      loadout: draft.value.loadout,
      painter: draft.value.painter,
      asset_bundle_url: draft.value.asset_bundle_url,
    }
    let saved = creating
      ? await createTtsFigurine(payload)
      : await updateTtsFigurine(editingId.value as number, payload)
    editingId.value = saved.id
    if (photoFile.value) {
      saved = await uploadTtsFigurinePhoto(saved.id, photoFile.value)
    } else if (removeExistingPhoto.value && saved.photo_url) {
      saved = await clearTtsFigurinePhoto(saved.id)
    }
    toast.success(creating ? 'Figurine ajoutée' : 'Figurine enregistrée')
    closePanel()
    await loadFigurines()
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible d’enregistrer la figurine')
    await loadFigurines()
  } finally {
    saving.value = false
  }
}

async function removeCurrent() {
  const id = editingId.value
  if (id == null) return
  const name = [draft.value.isc, draft.value.loadout].filter(Boolean).join(' · ') || 'cette figurine'
  if (!window.confirm(`Supprimer la figurine « ${name} » ?`)) return
  try {
    await deleteTtsFigurine(id)
    closePanel()
    toast.success('Figurine supprimée')
    await loadFigurines()
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible de supprimer la figurine')
  }
}

function pickIsc(value: string) {
  draft.value.isc = value
  iscFocused.value = false
}

function pickLoadout(value: string) {
  draft.value.loadout = value
  loadoutFocused.value = false
}

function blurIsc() {
  window.setTimeout(() => {
    iscFocused.value = false
  }, 150)
}

function blurLoadout() {
  window.setTimeout(() => {
    loadoutFocused.value = false
  }, 150)
}

watch(panelOpen, (open) => setCustomSide(open), { immediate: true })

onMounted(async () => {
  await loadFigurines()
  try {
    labels.value = await fetchTtsProfileLabels()
  } catch {
    labels.value = []
  }
})

onBeforeUnmount(() => {
  setCustomSide(false)
  revokePreview()
})
</script>

<template>
  <Teleport defer to="#app-side-panel">
    <Card
      v-if="panelOpen"
      class="neon-panel flex h-full max-h-full min-h-0 flex-col overflow-hidden"
      @paste="onPaste"
    >
      <CardHeader class="shrink-0 pb-3">
        <div class="flex items-start justify-between gap-2">
          <div class="min-w-0">
            <CardTitle>{{ editingId == null ? 'Nouvelle figurine' : draft.isc || 'Figurine' }}</CardTitle>
            <CardDescription>Colle une capture dans cette fiche (Ctrl+V).</CardDescription>
          </div>
          <Button type="button" variant="ghost" size="sm" @click="closePanel">Fermer</Button>
        </div>
      </CardHeader>
      <CardContent class="min-h-0 flex-1 space-y-4 overflow-y-auto pt-0">
        <form class="space-y-4" @submit.prevent="save">
          <div class="space-y-1.5">
            <Label for="fig-isc">ISC</Label>
            <Input
              id="fig-isc"
              v-model="draft.isc"
              autocomplete="off"
              required
              @focus="iscFocused = true"
              @blur="blurIsc"
            />
            <div v-if="showIscOptions" class="flex flex-wrap gap-1">
              <Button
                v-for="option in iscOptions"
                :key="option"
                type="button"
                variant="outline"
                size="xs"
                @mousedown.prevent="pickIsc(option)"
              >
                {{ option }}
              </Button>
            </div>
          </div>

          <div class="space-y-1.5">
            <Label for="fig-loadout">Loadout</Label>
            <Input
              id="fig-loadout"
              v-model="draft.loadout"
              autocomplete="off"
              @focus="loadoutFocused = true"
              @blur="blurLoadout"
            />
            <div v-if="showLoadoutOptions" class="flex flex-wrap gap-1">
              <Button
                v-for="option in loadoutOptions"
                :key="option"
                type="button"
                variant="outline"
                size="xs"
                @mousedown.prevent="pickLoadout(option)"
              >
                {{ option }}
              </Button>
            </div>
          </div>

          <div class="space-y-1.5">
            <Label for="fig-painter">Peintre</Label>
            <Input id="fig-painter" v-model="draft.painter" autocomplete="off" />
          </div>

          <div class="space-y-1.5">
            <Label for="fig-bundle">Asset bundle</Label>
            <Input
              id="fig-bundle"
              v-model="draft.asset_bundle_url"
              type="text"
              inputmode="url"
              autocomplete="off"
              spellcheck="false"
              placeholder="https://"
            />
          </div>

          <div class="space-y-1.5">
            <Label>Photo</Label>
            <button
              type="button"
              class="flex min-h-36 w-full flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border bg-muted/20 px-3 py-4 text-center text-sm text-muted-foreground"
              @click="fileInput?.click()"
              @dragover.prevent
              @drop.prevent="onDrop"
            >
              <img
                v-if="previewSrc"
                :src="previewSrc"
                alt="Aperçu de la figurine"
                class="max-h-48 w-full object-contain"
              />
              <span v-else>Colle une capture, ou clique pour choisir un fichier.</span>
            </button>
            <input
              ref="fileInput"
              type="file"
              accept="image/png,image/jpeg,image/gif,image/webp"
              class="hidden"
              @change="onFileChange"
            />
            <Button
              v-if="previewSrc"
              type="button"
              variant="ghost"
              size="xs"
              @click="forgetPhoto"
            >
              Retirer la photo
            </Button>
          </div>

          <div class="flex flex-wrap gap-2">
            <Button type="submit" :disabled="saving">
              {{ saving ? 'Enregistrement…' : 'Enregistrer' }}
            </Button>
            <Button
              v-if="editingId != null"
              type="button"
              variant="destructive"
              :disabled="saving"
              @click="removeCurrent"
            >
              Supprimer
            </Button>
          </div>
        </form>
      </CardContent>
    </Card>
  </Teleport>

  <div class="space-y-4">
    <div class="flex flex-wrap items-center gap-2">
      <Input
        v-model="search"
        class="max-w-xs"
        placeholder="ISC, loadout, peintre…"
        autocomplete="off"
      />
      <Button type="button" @click="openCreate">Ajouter une figurine</Button>
    </div>

    <Alert v-if="loadError" variant="destructive" class="neon-panel-accent">
      <AlertTitle>Chargement impossible</AlertTitle>
      <AlertDescription>{{ loadError }}</AlertDescription>
    </Alert>

    <p v-else-if="loading" class="text-sm text-muted-foreground">Chargement...</p>

    <p v-else-if="!filtered.length" class="text-sm text-muted-foreground">
      {{ figurines.length ? 'Aucune figurine ne correspond.' : 'Aucune figurine pour l’instant.' }}
    </p>

    <div v-else class="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
      <article
        v-for="figurine in filtered"
        :key="figurine.id"
        class="neon-panel overflow-hidden"
      >
        <button type="button" class="block w-full text-left" @click="openEdit(figurine)">
          <img
            v-if="figurine.photo_url"
            :src="photoSrc(figurine)"
            :alt="figurine.isc"
            class="aspect-[4/3] w-full bg-muted/30 object-contain"
            loading="lazy"
            decoding="async"
          />
          <div
            v-else
            class="flex aspect-[4/3] items-center justify-center bg-muted/20 text-sm text-muted-foreground"
          >
            Sans photo
          </div>
          <div class="space-y-1 px-3 py-2">
            <p class="font-medium">{{ figurine.isc }}</p>
            <p class="text-sm text-muted-foreground">{{ figurine.loadout || '—' }}</p>
            <p v-if="figurine.painter" class="text-sm">{{ figurine.painter }}</p>
          </div>
        </button>
        <div v-if="figurine.asset_bundle_url" class="px-3 pb-3">
          <a
            :href="figurine.asset_bundle_url"
            class="text-sm text-primary underline-offset-4 hover:underline"
            target="_blank"
            rel="noreferrer"
          >
            Asset bundle
          </a>
        </div>
      </article>
    </div>
  </div>
</template>
