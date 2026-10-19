<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Download } from '@lucide/vue'
import { toast } from 'vue-sonner'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import ArmyLogo from '@/components/ArmyLogo.vue'
import ColorPicker from '@/components/ColorPicker.vue'
import PlayerPicker from '@/components/PlayerPicker.vue'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { useAppSidePanel } from '@/composables/useAppSidePanel'
import { downloadBlob } from '@/lib/scenarioDocument'
import {
  clearTtsFigurinePhoto,
  createTtsFigurine,
  deleteTtsFigurine,
  fetchTtsFigurineLabels,
  fetchTtsSectorialColor,
  fetchTtsFigurineLoadouts,
  fetchTtsFigurineObject,
  fetchTtsFigurines,
  updateTtsFigurine,
  uploadTtsFigurinePhoto,
} from '@/lib/api'
import { withBase } from '@/lib/basePath'
import type { TtsFigurine, TtsFigurineLabel, TtsFigurineLoadout, TtsFigurineWrite } from '@/types/elo'

type FigurineLabel = {
  isc: string
  loadout: string
  silhouette: string
}

type Draft = TtsFigurineWrite

const props = defineProps<{
  armyId?: string
}>()

const emit = defineEmits<{
  saved: []
}>()

const { setCustomSide } = useAppSidePanel()
const { canEditContent } = useAdminEditMode()

const figurines = ref<TtsFigurine[]>([])
const labels = ref<FigurineLabel[]>([])
const search = defineModel<string>('search', { default: '' })
const painterFilter = defineModel<string>('painter', { default: '' })
const loading = ref(true)
const loadError = ref('')
const saving = ref(false)
const panel = ref<'edit' | 'download' | null>(null)
const editingId = ref<number | null>(null)
const downloadTarget = ref<TtsFigurine | null>(null)
const loadouts = ref<TtsFigurineLoadout[]>([])
const loadoutsLoading = ref(false)
const loadoutsError = ref('')
const selectedLoadoutKey = ref('')
const color = ref('FFFFFF')
const sectorialColor = ref('FFFFFF')
const downloading = ref(false)
const draft = ref<Draft>(emptyDraft())
const photoFile = ref<File | null>(null)
const photoPreview = ref('')
const existingPhoto = ref('')
const removeExistingPhoto = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)

const painterOptions = computed(() =>
  uniqueSorted(figurines.value.map((figurine) => figurine.painter)),
)

const inScope = computed(() => {
  const painter = painterFilter.value.trim().toLowerCase()
  let rows = figurines.value
  if (props.armyId) {
    const known = new Set(labels.value.map((label) => label.isc.trim().toLowerCase()))
    rows = rows.filter((figurine) => known.has(figurine.isc.trim().toLowerCase()))
  } else if (!painter) {
    return []
  }
  if (painter) {
    rows = rows.filter((figurine) => figurine.painter.trim().toLowerCase() === painter)
  }
  return rows
})

const filtered = computed(() => {
  const needle = search.value.trim().toLowerCase()
  if (!needle) return inScope.value
  return inScope.value.filter((figurine) =>
    [figurine.isc, figurine.loadout, figurine.painter, figurine.asset_bundle_url]
      .join(' ')
      .toLowerCase()
      .includes(needle),
  )
})

const iscPickerOptions = computed(() => uniqueSorted(labels.value.map((label) => label.isc)))

const loadoutPickerOptions = computed(() => {
  const isc = draft.value.isc.trim().toLowerCase()
  if (!isc) return []
  return uniqueSorted(
    labels.value
      .filter((label) => label.isc.trim().toLowerCase() === isc)
      .map((label) => label.loadout)
      .filter((loadout) => loadout.trim() !== ''),
  )
})

const previewSrc = computed(() => photoPreview.value || (!removeExistingPhoto.value ? existingPhoto.value : ''))
const panelOpen = computed(() => panel.value != null)
const selectedLoadout = computed(() =>
  loadouts.value.find((loadout) => loadoutKey(loadout) === selectedLoadoutKey.value),
)

function emptyDraft(): Draft {
  return { isc: '', loadout: '', painter: '', asset_bundle_url: '' }
}

function uniqueSorted(values: string[]) {
  const seen = new Set<string>()
  const options: { value: string; label: string }[] = []
  for (const value of values) {
    const trimmed = value.trim()
    if (!trimmed) continue
    const key = trimmed.toLowerCase()
    if (seen.has(key)) continue
    seen.add(key)
    options.push({ value: trimmed, label: trimmed })
  }
  options.sort((left, right) => left.label.localeCompare(right.label, 'fr', { sensitivity: 'base' }))
  return options
}

function resetCreateForm() {
  editingId.value = null
  draft.value = emptyDraft()
  draft.value.painter = painterFilter.value.trim()
  existingPhoto.value = ''
  removeExistingPhoto.value = false
  photoFile.value = null
  revokePreview()
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

function silhouetteOf(
  profile: Pick<TtsFigurineLabel, 'marker_name' | 'silhouette_size' | 'silhouette_type'>,
) {
  const marker = profile.marker_name?.trim()
  if (marker) return marker
  const size = profile.silhouette_size?.trim()
  if (!size) return ''
  const kind = profile.silhouette_type?.trim()
  if (kind && kind.toLowerCase() !== 'normale') return `S${size} · ${kind}`
  return `S${size}`
}

function loadoutKey(option: TtsFigurineLoadout) {
  return option.form_id != null ? `f:${option.form_id}` : `p:${option.profile_id}`
}

function figurineSilhouette(figurine: TtsFigurine) {
  const isc = figurine.isc.trim().toLowerCase()
  const loadout = figurine.loadout.trim().toLowerCase()
  const sameIsc = labels.value.filter((label) => label.isc.trim().toLowerCase() === isc && label.silhouette)
  const exact = sameIsc.find((label) => label.loadout.trim().toLowerCase() === loadout)
  return (exact ?? sameIsc[0])?.silhouette ?? ''
}

function figurineSectorials(figurine: TtsFigurine) {
  return figurine.sectorial_ids ?? []
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
  resetCreateForm()
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
  selectedLoadoutKey.value = ''
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
    const match = rows.find((row) => sameText(row.loadout, figurine.loadout)) ?? rows[0]
    selectedLoadoutKey.value = match ? loadoutKey(match) : ''
  } catch (error) {
    if (downloadTarget.value?.id !== figurine.id) return
    loadoutsError.value = error instanceof Error ? error.message : 'Impossible de charger les loadouts'
  } finally {
    if (downloadTarget.value?.id === figurine.id) loadoutsLoading.value = false
  }
}

async function downloadFigurine() {
  const figurine = downloadTarget.value
  const option = selectedLoadout.value
  const armyId = Number(props.armyId)
  if (!figurine || !option || !armyId || downloading.value) return
  downloading.value = true
  try {
    const object = await fetchTtsFigurineObject(figurine.id, {
      armyId,
      color: color.value,
      formId: option.form_id ?? undefined,
      profileId: option.form_id == null ? option.profile_id : undefined,
    })
    const loadout = option.loadout || figurine.loadout
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
  if (!draft.value.isc.trim()) {
    toast.error('Choisis un ISC.')
    return
  }
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
    if (photoFile.value) {
      saved = await uploadTtsFigurinePhoto(saved.id, photoFile.value)
    } else if (removeExistingPhoto.value && saved.photo_url) {
      saved = await clearTtsFigurinePhoto(saved.id)
    }
    toast.success(creating ? 'Figurine ajoutée' : 'Figurine enregistrée')
    if (creating) {
      resetCreateForm()
      panel.value = 'edit'
    } else {
      editingId.value = saved.id
      draft.value = {
        isc: saved.isc,
        loadout: saved.loadout,
        painter: saved.painter,
        asset_bundle_url: saved.asset_bundle_url,
      }
      existingPhoto.value = fullPhotoSrc(saved)
      removeExistingPhoto.value = false
      photoFile.value = null
      revokePreview()
    }
    await loadFigurines()
    emit('saved')
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
    emit('saved')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible de supprimer la figurine')
  }
}

watch(panelOpen, (open) => setCustomSide(open), { immediate: true })

watch(canEditContent, (canEdit) => {
  if (!canEdit && panel.value === 'edit' && editingId.value == null) closePanel()
})

watch(
  () => draft.value.isc,
  (isc) => {
    const key = isc.trim().toLowerCase()
    if (!key) {
      draft.value.loadout = ''
      return
    }
    const allowed = new Set(
      labels.value
        .filter((label) => label.isc.trim().toLowerCase() === key)
        .map((label) => label.loadout.trim().toLowerCase()),
    )
    if (draft.value.loadout && !allowed.has(draft.value.loadout.trim().toLowerCase())) {
      draft.value.loadout = ''
    }
  },
)

watch(() => props.armyId, () => {
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
    const rows = await fetchTtsFigurineLabels(armyId)
    if (Number(props.armyId) !== armyId) return
    labels.value = rows.map((row) => ({
      isc: row.isc,
      loadout: row.loadout,
      silhouette: silhouetteOf(row),
    }))
  } catch {
    if (Number(props.armyId) !== armyId) return
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
                :class="loadoutKey(option) === selectedLoadoutKey ? 'border-primary' : 'border-border'"
              >
                <input
                  v-model="selectedLoadoutKey"
                  class="mt-1"
                  type="radio"
                  name="figurine-loadout"
                  :value="loadoutKey(option)"
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
          <Button type="submit" :disabled="downloading || !downloadTarget.asset_bundle_url || !selectedLoadoutKey">
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
            <Label>ISC</Label>
            <PlayerPicker
              :model-value="draft.isc || undefined"
              :options="iscPickerOptions"
              :disabled="!canEditContent"
              placeholder="Choisir un ISC"
              empty-message="Aucun ISC dans cette sectorielle."
              @update:model-value="draft.isc = $event ?? ''"
            />
          </div>

          <div class="space-y-1.5">
            <Label>Loadout</Label>
            <PlayerPicker
              :model-value="draft.loadout || undefined"
              :options="loadoutPickerOptions"
              :disabled="!canEditContent || !draft.isc"
              placeholder="Choisir un loadout"
              empty-message="Aucun loadout pour cet ISC."
              @update:model-value="draft.loadout = $event ?? ''"
            />
          </div>

          <div class="space-y-1.5">
            <Label>Peintre</Label>
            <PlayerPicker
              :model-value="draft.painter || undefined"
              :options="painterOptions"
              :disabled="!canEditContent"
              allow-custom
              placeholder="Choisir ou saisir un peintre"
              empty-message="Aucun peintre connu."
              @update:model-value="draft.painter = $event ?? ''"
            />
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

    <p v-else-if="!armyId && !painterFilter.trim()" class="text-sm text-muted-foreground">
      Choisis une sectorielle ou un peintre pour afficher les figurines.
    </p>

    <p v-else-if="!filtered.length" class="text-sm text-muted-foreground">
      {{
        inScope.length
          ? 'Aucune figurine ne correspond.'
          : armyId
            ? 'Aucune figurine pour cette sectorielle.'
            : 'Aucune figurine pour ce peintre.'
      }}
    </p>

    <div v-else class="grid grid-cols-2 gap-3 lg:grid-cols-4">
      <article
        v-for="figurine in filtered"
        :key="figurine.id"
        class="neon-panel min-w-0 overflow-hidden"
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
        <button type="button" class="block w-full min-w-0 px-3 pt-2 pb-3 text-left" @click="openEdit(figurine)">
          <div class="min-w-0 space-y-1">
            <p class="truncate font-medium" :title="figurine.isc">{{ figurine.isc }}</p>
            <p class="truncate text-sm text-muted-foreground" :title="figurine.loadout || undefined">
              {{ figurine.loadout || '—' }}
            </p>
            <div
              v-if="figurineSilhouette(figurine) || figurineSectorials(figurine).length || figurine.painter"
              class="flex min-w-0 items-center gap-2 text-sm"
            >
              <span
                class="min-w-0 flex-1 truncate text-muted-foreground"
                :title="figurineSilhouette(figurine) || undefined"
              >
                {{ figurineSilhouette(figurine) }}
              </span>
              <div
                v-if="figurineSectorials(figurine).length || figurine.painter"
                class="flex min-w-0 shrink-0 items-center justify-end gap-1.5"
              >
                <div
                  v-if="figurineSectorials(figurine).length"
                  class="flex max-w-[7.5rem] flex-wrap items-center justify-end gap-0.5"
                >
                  <ArmyLogo
                    v-for="armyId in figurineSectorials(figurine)"
                    :key="armyId"
                    :army-id="armyId"
                    class="!size-4"
                  />
                </div>
                <span
                  v-if="figurine.painter"
                  class="min-w-0 max-w-[7rem] truncate text-right"
                  :title="figurine.painter"
                >
                  {{ figurine.painter }}
                </span>
              </div>
            </div>
          </div>
        </button>
      </article>
    </div>
  </div>
</template>
