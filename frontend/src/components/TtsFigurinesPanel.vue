<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Download } from '@lucide/vue'
import { toast } from 'vue-sonner'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import ColorPicker from '@/components/ColorPicker.vue'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { useAppSidePanel } from '@/composables/useAppSidePanel'
import { downloadBlob } from '@/lib/scenarioDocument'
import {
  clearTtsFigurinePhoto,
  createTtsFigurine,
  deleteTtsFigurine,
  fetchTtsSectorialColor,
  fetchTtsFigurineLoadouts,
  fetchTtsFigurineObject,
  fetchTtsFigurines,
  fetchTtsProfiles,
  updateTtsFigurine,
  uploadTtsFigurinePhoto,
} from '@/lib/api'
import { withBase } from '@/lib/basePath'
import type { TtsFigurine, TtsFigurineLoadout, TtsFigurineWrite, TtsProfileLabel } from '@/types/elo'

type Draft = TtsFigurineWrite

const props = defineProps<{
  armyId?: string
}>()

const { setCustomSide } = useAppSidePanel()
const { canEditContent } = useAdminEditMode()

const figurines = ref<TtsFigurine[]>([])
const labels = ref<TtsProfileLabel[]>([])
const search = defineModel<string>('search', { default: '' })
const loading = ref(true)
const loadError = ref('')
const saving = ref(false)
const panel = ref<'edit' | 'download' | null>(null)
const editingId = ref<number | null>(null)
const downloadTarget = ref<TtsFigurine | null>(null)
const loadouts = ref<TtsFigurineLoadout[]>([])
const loadoutsLoading = ref(false)
const loadoutsError = ref('')
const selectedProfileId = ref('')
const color = ref('FFFFFF')
const sectorialColor = ref('FFFFFF')
const downloading = ref(false)
const draft = ref<Draft>(emptyDraft())
const photoFile = ref<File | null>(null)
const photoPreview = ref('')
const existingPhoto = ref('')
const removeExistingPhoto = ref(false)
const iscFocused = ref(false)
const loadoutFocused = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)

const inSectorial = computed(() => {
  if (!props.armyId) return []
  const known = new Set(labels.value.map((label) => label.isc.trim().toLowerCase()))
  return figurines.value.filter((figurine) => known.has(figurine.isc.trim().toLowerCase()))
})

const filtered = computed(() => {
  const needle = search.value.trim().toLowerCase()
  if (!needle) return inSectorial.value
  return inSectorial.value.filter((figurine) =>
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
const panelOpen = computed(() => panel.value != null)
const selectedLoadout = computed(() =>
  loadouts.value.find((loadout) => String(loadout.profile_id) === selectedProfileId.value),
)

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
  if (!canEditContent.value) return
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

function sameText(left: string, right: string) {
  return left.trim().toLowerCase() === right.trim().toLowerCase()
}

function loadoutTitle(option: TtsFigurineLoadout) {
  const name = option.loadout.trim() || '—'
  const siblings = loadouts.value.filter((loadout) => loadout.loadout === option.loadout)
  const factions = new Set(siblings.map((loadout) => loadout.faction_label))
  if (factions.size > 1 && option.faction_label) return `${name} · ${option.faction_label}`
  return name
}

function downloadFilename(isc: string, loadout: string) {
  const raw = [isc.trim(), loadout.trim()].filter(Boolean).join(' - ')
  const safe = raw.replace(/[\\/:*?"<>|]/g, ' ').replace(/\s+/g, ' ').trim()
  return `${safe || 'figurine'}.json`
}

function openCreate() {
  if (!canEditContent.value) return
  editingId.value = null
  draft.value = emptyDraft()
  existingPhoto.value = ''
  removeExistingPhoto.value = false
  photoFile.value = null
  revokePreview()
  panel.value = 'edit'
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
  panel.value = 'edit'
}

async function openDownload(figurine: TtsFigurine) {
  downloadTarget.value = figurine
  loadouts.value = []
  loadoutsError.value = ''
  selectedProfileId.value = ''
  panel.value = 'download'
  loadoutsLoading.value = true
  try {
    const armyId = Number(props.armyId)
    if (!armyId) {
      loadoutsError.value = 'Choisis une sectorielle.'
      return
    }
    const rows = await fetchTtsFigurineLoadouts(figurine.id, armyId)
    if (downloadTarget.value?.id !== figurine.id) return
    loadouts.value = rows
    const match = rows.find((row) => sameText(row.loadout, figurine.loadout))
    selectedProfileId.value = String((match ?? rows[0])?.profile_id ?? '')
  } catch (error) {
    if (downloadTarget.value?.id !== figurine.id) return
    loadoutsError.value = error instanceof Error ? error.message : 'Impossible de charger les loadouts'
  } finally {
    if (downloadTarget.value?.id === figurine.id) loadoutsLoading.value = false
  }
}

async function downloadFigurine() {
  const figurine = downloadTarget.value
  const profileId = Number(selectedProfileId.value)
  const armyId = Number(props.armyId)
  if (!figurine || !profileId || !armyId || downloading.value) return
  downloading.value = true
  try {
    const object = await fetchTtsFigurineObject(figurine.id, profileId, armyId, color.value)
    const loadout = selectedLoadout.value?.loadout ?? figurine.loadout
    downloadBlob(
      new Blob([JSON.stringify(object, null, 2)], { type: 'application/json' }),
      downloadFilename(figurine.isc, loadout),
    )
    toast.success('Figurine téléchargée')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible de télécharger la figurine')
  } finally {
    downloading.value = false
  }
}

function closePanel() {
  panel.value = null
  downloadTarget.value = null
  photoFile.value = null
  revokePreview()
}

function forgetPhoto() {
  if (!canEditContent.value) return
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
  if (!canEditContent.value) return
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
  if (!canEditContent.value) return
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

watch(canEditContent, (canEdit) => {
  if (!canEdit && panel.value === 'edit' && editingId.value == null) closePanel()
})

watch(() => props.armyId, () => {
  search.value = ''
  void loadLabels()
  void loadFactionColor()
  if (panel.value === 'download' && downloadTarget.value) {
    void openDownload(downloadTarget.value)
  }
}, { immediate: true })

async function loadFactionColor() {
  const armyId = Number(props.armyId)
  if (!armyId) return
  try {
    const next = (await fetchTtsSectorialColor(armyId)).toUpperCase()
    if (Number(props.armyId) !== armyId) return
    sectorialColor.value = next
    color.value = next
  } catch {
    if (Number(props.armyId) !== armyId) return
    sectorialColor.value = 'FFFFFF'
    color.value = 'FFFFFF'
  }
}

async function loadLabels() {
  const armyId = Number(props.armyId)
  if (!armyId) {
    labels.value = []
    return
  }
  try {
    const profiles = await fetchTtsProfiles({ armyId, includeFaction: false })
    labels.value = profiles.map((profile) => ({
      isc: profile.isc,
      loadout: profile.loadout,
    }))
  } catch {
    labels.value = []
  }
}

onMounted(async () => {
  await loadFigurines()
})

defineExpose({ openCreate })

onBeforeUnmount(() => {
  setCustomSide(false)
  revokePreview()
})
</script>

<template>
  <Teleport defer to="#app-side-panel">
    <Card
      v-if="panel === 'download' && downloadTarget"
      class="neon-panel flex h-full max-h-full min-h-0 flex-col overflow-hidden"
    >
      <CardHeader class="shrink-0 pb-3">
        <div class="flex items-start justify-between gap-2">
          <div class="min-w-0">
            <CardTitle>{{ downloadTarget.isc }}</CardTitle>
            <CardDescription>
              Choisis un loadout de cet ISC. L’état 2 du fichier est la silhouette de ce profil.
            </CardDescription>
          </div>
          <Button type="button" variant="ghost" size="sm" @click="closePanel">Fermer</Button>
        </div>
      </CardHeader>
      <CardContent class="min-h-0 flex-1 space-y-4 overflow-y-auto pt-0">
        <Alert v-if="!downloadTarget.asset_bundle_url" variant="destructive">
          <AlertTitle>Asset bundle manquant</AlertTitle>
          <AlertDescription>Cette figurine n’a pas encore de lien d’asset bundle.</AlertDescription>
        </Alert>
        <Alert v-else-if="loadoutsError" variant="destructive">
          <AlertTitle>Loadouts indisponibles</AlertTitle>
          <AlertDescription>{{ loadoutsError }}</AlertDescription>
        </Alert>
        <p v-else-if="loadoutsLoading" class="text-sm text-muted-foreground">Chargement des loadouts…</p>
        <p v-else-if="!loadouts.length" class="text-sm text-muted-foreground">
          Aucun loadout connu pour cet ISC.
        </p>
        <form v-else class="space-y-4" @submit.prevent="downloadFigurine">
          <div class="space-y-2">
            <div class="flex items-center justify-between gap-2">
              <p class="text-sm font-medium">Couleur</p>
              <Button
                v-if="color.toUpperCase() !== sectorialColor.toUpperCase()"
                type="button"
                variant="ghost"
                size="sm"
                @click="color = sectorialColor"
              >
                Couleur de la sectorielle
              </Button>
            </div>
            <ColorPicker v-model="color" />
          </div>
          <fieldset class="space-y-2">
            <legend class="text-sm font-medium">Loadout</legend>
            <div class="max-h-[28rem] space-y-1 overflow-y-auto">
              <label
                v-for="option in loadouts"
                :key="option.profile_id"
                class="flex cursor-pointer gap-2 rounded-md border px-2 py-1.5 text-sm"
                :class="String(option.profile_id) === selectedProfileId ? 'border-primary' : 'border-border'"
              >
                <input
                  v-model="selectedProfileId"
                  class="mt-1"
                  type="radio"
                  name="figurine-loadout"
                  :value="String(option.profile_id)"
                />
                <span class="min-w-0">
                  <span class="block break-words">{{ loadoutTitle(option) }}</span>
                  <span class="block text-muted-foreground">État 2 · {{ option.marker_name }}</span>
                  <span v-if="option.differences" class="block break-words text-muted-foreground">
                    {{ option.differences }}
                  </span>
                  <span v-if="option.extra_forms" class="block text-muted-foreground">
                    {{ option.extra_forms }} forme{{ option.extra_forms > 1 ? 's' : '' }} ensuite, avec leur silhouette
                  </span>
                </span>
              </label>
            </div>
          </fieldset>
          <Button type="submit" :disabled="downloading || !downloadTarget.asset_bundle_url || !selectedProfileId">
            {{ downloading ? 'Téléchargement…' : 'Télécharger' }}
          </Button>
        </form>
      </CardContent>
    </Card>

    <Card
      v-else-if="panel === 'edit'"
      class="neon-panel flex h-full max-h-full min-h-0 flex-col overflow-hidden"
      @paste="onPaste"
    >
      <CardHeader class="shrink-0 pb-3">
        <div class="flex items-start justify-between gap-2">
          <div class="min-w-0">
            <CardTitle>{{ editingId == null ? 'Nouvelle figurine' : draft.isc || 'Figurine' }}</CardTitle>
            <CardDescription v-if="canEditContent">Colle une capture dans cette fiche (Ctrl+V).</CardDescription>
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
              :disabled="!canEditContent"
              @focus="iscFocused = true"
              @blur="blurIsc"
            />
            <div v-if="canEditContent && showIscOptions" class="flex flex-wrap gap-1">
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
              :disabled="!canEditContent"
              @focus="loadoutFocused = true"
              @blur="blurLoadout"
            />
            <div v-if="canEditContent && showLoadoutOptions" class="flex flex-wrap gap-1">
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
            <Input id="fig-painter" v-model="draft.painter" autocomplete="off" :disabled="!canEditContent" />
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
              :disabled="!canEditContent"
            />
          </div>

          <div class="space-y-1.5">
            <Label>Photo</Label>
            <button
              type="button"
              class="flex min-h-36 w-full flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border bg-muted/20 px-3 py-4 text-center text-sm text-muted-foreground"
              :disabled="!canEditContent"
              @click="canEditContent && fileInput?.click()"
              @dragover.prevent
              @drop.prevent="onDrop"
            >
              <img
                v-if="previewSrc"
                :src="previewSrc"
                alt="Aperçu de la figurine"
                class="max-h-48 w-full object-contain"
              />
              <span v-else-if="canEditContent">Colle une capture, ou clique pour choisir un fichier.</span>
              <span v-else>Aucune photo.</span>
            </button>
            <input
              ref="fileInput"
              type="file"
              accept="image/png,image/jpeg,image/gif,image/webp"
              class="hidden"
              @change="onFileChange"
            />
            <Button
              v-if="canEditContent && previewSrc"
              type="button"
              variant="ghost"
              size="xs"
              @click="forgetPhoto"
            >
              Retirer la photo
            </Button>
          </div>

          <div v-if="canEditContent" class="flex flex-wrap gap-2">
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
    <Alert v-if="loadError" variant="destructive" class="neon-panel-accent">
      <AlertTitle>Chargement impossible</AlertTitle>
      <AlertDescription>{{ loadError }}</AlertDescription>
    </Alert>

    <p v-else-if="loading" class="text-sm text-muted-foreground">Chargement...</p>

    <p v-else-if="!armyId" class="text-sm text-muted-foreground">
      Choisis une sectorielle pour afficher ses figurines.
    </p>

    <p v-else-if="!filtered.length" class="text-sm text-muted-foreground">
      {{ inSectorial.length ? 'Aucune figurine ne correspond.' : 'Aucune figurine pour cette sectorielle.' }}
    </p>

    <div v-else class="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
      <article
        v-for="figurine in filtered"
        :key="figurine.id"
        class="neon-panel overflow-hidden"
      >
        <div class="relative">
          <button type="button" class="block w-full" @click="openEdit(figurine)">
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
          </button>
          <button
            type="button"
            class="absolute top-2 right-2 flex size-8 items-center justify-center rounded-md bg-black text-white"
            aria-label="Télécharger"
            @click="openDownload(figurine)"
          >
            <Download class="size-4" />
          </button>
        </div>
        <button type="button" class="block w-full px-3 pt-2 pb-3 text-left" @click="openEdit(figurine)">
          <div class="space-y-1">
            <p class="font-medium">{{ figurine.isc }}</p>
            <p class="text-sm text-muted-foreground">{{ figurine.loadout || '—' }}</p>
            <p v-if="figurine.painter" class="text-sm">{{ figurine.painter }}</p>
          </div>
        </button>
      </article>
    </div>
  </div>
</template>
