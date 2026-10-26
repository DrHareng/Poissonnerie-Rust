<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { Download } from '@lucide/vue'
import { toast } from 'vue-sonner'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
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
  fetchTtsFigurineMeta,
  fetchTtsProfileLabels,
  fetchTtsSectorialColor,
  fetchTtsFigurineLoadouts,
  fetchTtsFigurineObject,
  fetchTtsFigurines,
  updateTtsFigurine,
  uploadTtsFigurinePhoto,
} from '@/lib/api'
import { withBase } from '@/lib/basePath'
import type { TtsFigurine, TtsFigurineLabel, TtsFigurineLoadout, TtsFigurineWrite } from '@/types/elo'

const PAGE_SIZE = 20
const SEARCH_DEBOUNCE_MS = 200

type FigurineLabel = {
  /** null si les libellés viennent du catalogue global (sans id). */
  tts_loadout_id: number | null
  nom: string
  loadout: string
  silhouette: string
}

type Draft = Omit<TtsFigurineWrite, 'nom' | 'loadout'> & {
  nom: string
  loadout: string
  tts_loadout_id: number | null
}

const props = defineProps<{
  armyId?: string
  /** Admin : ne garder que les figs avec infos manquantes. */
  incompleteOnly?: boolean
}>()

const emit = defineEmits<{
  saved: []
}>()

const { setCustomSide } = useAppSidePanel()
const { canEditContent } = useAdminEditMode()

const figurines = ref<TtsFigurine[]>([])
const labels = ref<FigurineLabel[]>([])
const knownPainters = ref<string[]>([])
const search = defineModel<string>('search', { default: '' })
const painterFilter = defineModel<string>('painter', { default: '' })
const loading = ref(true)
const loadingMore = ref(false)
const loadError = ref('')
const total = ref(0)
const saving = ref(false)
const scrollEl = ref<HTMLElement | null>(null)
const loadMoreSentinel = ref<HTMLElement | null>(null)
let searchDebounce: ReturnType<typeof setTimeout> | null = null
let loadGeneration = 0
let loadMoreObserver: IntersectionObserver | null = null
const panel = ref<'edit' | 'download' | null>(null)
const editingId = ref<number | null>(null)
/** Lien d'origine de la figurine en édition (conservé si nom/loadout ne changent pas). */
const originalLink = ref<{ id: number | null; nom: string; loadout: string } | null>(null)
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

const hasMore = computed(() => figurines.value.length < total.value)

const painterOptions = computed(() => uniqueSorted(knownPainters.value))

const iscPickerOptions = computed(() => uniqueSorted(labels.value.map((label) => label.nom)))

const loadoutPickerOptions = computed(() => {
  const isc = draft.value.nom.trim().toLowerCase()
  if (!isc) return []
  return uniqueSorted(
    labels.value
      .filter((label) => label.nom.trim().toLowerCase() === isc)
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
  return { tts_loadout_id: null, nom: '', loadout: '', painter: '', asset_bundle_url: '' }
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
  originalLink.value = null
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
  const isc = figurine.nom.trim().toLowerCase()
  const loadout = figurine.loadout.trim().toLowerCase()
  const sameIsc = labels.value.filter((label) => label.nom.trim().toLowerCase() === isc && label.silhouette)
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
  originalLink.value = {
    id: figurine.tts_loadout_id,
    nom: figurine.nom,
    loadout: figurine.loadout,
  }
  draft.value = {
    tts_loadout_id: figurine.tts_loadout_id,
    nom: figurine.nom,
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
      downloadFilename(figurine.nom, loadout),
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

async function loadPainterOptions() {
  try {
    knownPainters.value = (await fetchTtsFigurineMeta()).painters
  } catch {
    knownPainters.value = []
  }
}

async function loadFigurines(reset = true) {
  const generation = reset ? ++loadGeneration : loadGeneration
  if (reset) {
    loading.value = true
    loadError.value = ''
    figurines.value = []
    total.value = 0
  } else {
    if (loadingMore.value || loading.value || !hasMore.value) return
    loadingMore.value = true
  }
  try {
    const page = await fetchTtsFigurines({
      limit: PAGE_SIZE,
      offset: reset ? 0 : figurines.value.length,
      q: search.value,
      painter: painterFilter.value,
      armyId: props.armyId,
      // `incomplete` côté backend inclut déjà les figurines orphelines (unlinked).
      incomplete: props.incompleteOnly,
    })
    if (generation !== loadGeneration) return
    total.value = page.total
    figurines.value = reset ? page.items : [...figurines.value, ...page.items]
  } catch (error) {
    if (generation !== loadGeneration) return
    loadError.value = error instanceof Error ? error.message : 'Impossible de charger les figurines'
    if (reset) figurines.value = []
  } finally {
    if (generation === loadGeneration) {
      loading.value = false
      loadingMore.value = false
    }
  }
}

function scheduleReload() {
  if (searchDebounce != null) clearTimeout(searchDebounce)
  searchDebounce = setTimeout(() => {
    searchDebounce = null
    void loadFigurines(true)
  }, SEARCH_DEBOUNCE_MS)
}

function bindLoadMoreObserver() {
  loadMoreObserver?.disconnect()
  loadMoreObserver = null
  const root = scrollEl.value
  const target = loadMoreSentinel.value
  if (!root || !target || typeof IntersectionObserver === 'undefined') return
  loadMoreObserver = new IntersectionObserver(
    (entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        void loadFigurines(false)
      }
    },
    { root, rootMargin: '200px 0px' },
  )
  loadMoreObserver.observe(target)
}

async function save() {
  if (!canEditContent.value) return
  if (!draft.value.nom.trim()) {
    toast.error('Choisis un nom.')
    return
  }
  saving.value = true
  const creating = editingId.value == null
  try {
    const payload: TtsFigurineWrite = {
      tts_loadout_id: draft.value.tts_loadout_id,
      nom: draft.value.nom,
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
      originalLink.value = { id: saved.tts_loadout_id, nom: saved.nom, loadout: saved.loadout }
      draft.value = {
        tts_loadout_id: saved.tts_loadout_id,
        nom: saved.nom,
        loadout: saved.loadout,
        painter: saved.painter,
        asset_bundle_url: saved.asset_bundle_url,
      }
      existingPhoto.value = fullPhotoSrc(saved)
      removeExistingPhoto.value = false
      photoFile.value = null
      revokePreview()
    }
    await Promise.all([loadFigurines(true), loadPainterOptions()])
    emit('saved')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible d’enregistrer la figurine')
    await Promise.all([loadFigurines(true), loadPainterOptions()])
  } finally {
    saving.value = false
  }
}

async function removeCurrent() {
  if (!canEditContent.value) return
  const id = editingId.value
  if (id == null) return
  const name = [draft.value.nom, draft.value.loadout].filter(Boolean).join(' · ') || 'cette figurine'
  if (!window.confirm(`Supprimer la figurine « ${name} » ?`)) return
  try {
    await deleteTtsFigurine(id)
    closePanel()
    toast.success('Figurine supprimée')
    await Promise.all([loadFigurines(true), loadPainterOptions()])
    emit('saved')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible de supprimer la figurine')
  }
}

// Garde draft.tts_loadout_id aligné sur la paire nom + loadout choisie dans les libellés.
watch(
  [() => draft.value.nom, () => draft.value.loadout, labels],
  () => {
    const nom = draft.value.nom.trim().toLowerCase()
    const loadout = draft.value.loadout.trim().toLowerCase()
    if (!nom) {
      draft.value.tts_loadout_id = null
      return
    }
    const match = labels.value.find(
      (label) =>
        label.tts_loadout_id != null &&
        label.nom.trim().toLowerCase() === nom &&
        label.loadout.trim().toLowerCase() === loadout,
    )
    if (match) {
      draft.value.tts_loadout_id = match.tts_loadout_id
      return
    }
    // Libellés pas encore chargés ou catalogue global sans ids : ne pas casser le lien existant.
    if (labels.value.length === 0) return
    const original = originalLink.value
    if (original && sameText(original.nom, draft.value.nom) && sameText(original.loadout, draft.value.loadout)) {
      draft.value.tts_loadout_id = original.id
      return
    }
    if (labels.value.some((label) => label.tts_loadout_id != null)) {
      draft.value.tts_loadout_id = null
    }
  },
)

watch(panelOpen, (open) => setCustomSide(open), { immediate: true })

watch(canEditContent, (canEdit) => {
  if (!canEdit && panel.value === 'edit' && editingId.value == null) closePanel()
})

watch(
  [() => draft.value.nom, labels],
  () => {
    const key = draft.value.nom.trim().toLowerCase()
    if (!key) {
      draft.value.loadout = ''
      return
    }
    // Ne pas effacer le loadout tant que les libellés ne sont pas chargés
    // (ex. bascule sans sectorielle → catalogue global).
    if (labels.value.length === 0) return
    const options = loadoutPickerOptions.value
    if (options.length === 1) {
      const only = options[0].value
      if (draft.value.loadout !== only) draft.value.loadout = only
      return
    }
    const allowed = new Set(
      labels.value
        .filter((label) => label.nom.trim().toLowerCase() === key)
        .map((label) => label.loadout.trim().toLowerCase()),
    )
    if (allowed.size === 0) return
    if (draft.value.loadout && !allowed.has(draft.value.loadout.trim().toLowerCase())) {
      draft.value.loadout = ''
    }
  },
)

watch(() => props.armyId, () => {
  void loadLabels()
  void loadFactionColor()
  void loadFigurines(true)
  void loadPainterOptions()
  if (panel.value === 'download' && downloadTarget.value) {
    void openDownload(downloadTarget.value)
  }
}, { immediate: true })

watch([search, painterFilter, () => props.incompleteOnly], () => {
  scheduleReload()
})

watch(
  [scrollEl, loadMoreSentinel, hasMore],
  async () => {
    await nextTick()
    bindLoadMoreObserver()
  },
)

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
    try {
      const rows = await fetchTtsProfileLabels()
      if (Number(props.armyId)) return
      labels.value = rows.map((row) => ({
        tts_loadout_id: row.tts_loadout_id,
        nom: row.nom,
        loadout: row.loadout,
        silhouette: '',
      }))
    } catch {
      if (!Number(props.armyId)) labels.value = []
    }
    return
  }
  try {
    const rows = await fetchTtsFigurineLabels(armyId)
    if (Number(props.armyId) !== armyId) return
    labels.value = rows.map((row) => ({
      tts_loadout_id: row.tts_loadout_id,
      nom: row.nom,
      loadout: row.loadout,
      silhouette: silhouetteOf(row),
    }))
  } catch {
    if (Number(props.armyId) !== armyId) return
    labels.value = []
  }
}

defineExpose({ openCreate })

onBeforeUnmount(() => {
  if (searchDebounce != null) clearTimeout(searchDebounce)
  loadMoreObserver?.disconnect()
  loadMoreObserver = null
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
            <CardTitle>{{ downloadTarget.nom }}</CardTitle>
            <CardDescription>
              Choisis un loadout de ce nom. L’état 2 du fichier est la silhouette de ce profil.
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
          Aucun loadout connu pour ce nom.
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
            <CardTitle class="flex flex-wrap items-center gap-2">
              {{ editingId == null ? 'Nouvelle figurine' : draft.nom || 'Figurine' }}
              <Badge v-if="editingId != null && draft.tts_loadout_id == null" variant="destructive">
                Orpheline
              </Badge>
            </CardTitle>
            <CardDescription v-if="canEditContent">Colle une capture dans cette fiche (Ctrl+V).</CardDescription>
          </div>
          <Button type="button" variant="ghost" size="sm" @click="closePanel">Fermer</Button>
        </div>
      </CardHeader>
      <CardContent class="min-h-0 flex-1 overflow-y-auto pt-0">
        <form class="flex min-h-full flex-col gap-4" @submit.prevent="save">
          <div class="grid grid-cols-2 gap-3">
            <div class="min-w-0 space-y-1.5">
              <Label>Nom</Label>
              <PlayerPicker
                :model-value="draft.nom || undefined"
                :options="iscPickerOptions"
                :disabled="!canEditContent"
                placeholder="Choisir un nom"
                :empty-message="armyId ? 'Aucun nom dans cette sectorielle.' : 'Aucun nom connu.'"
                @update:model-value="draft.nom = $event ?? ''"
              />
            </div>
            <div class="min-w-0 space-y-1.5">
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
          </div>

          <div class="space-y-1.5">
            <Label>Loadout</Label>
            <PlayerPicker
              :model-value="draft.loadout || undefined"
              :options="loadoutPickerOptions"
              :disabled="!canEditContent || !draft.nom"
              placeholder="Choisir un loadout"
              empty-message="Aucun loadout pour ce nom."
              @update:model-value="draft.loadout = $event ?? ''"
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

          <div class="flex min-h-0 flex-1 flex-col space-y-1.5">
            <Label>Photo</Label>
            <button
              type="button"
              class="flex min-h-64 w-full flex-1 flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border bg-muted/20 px-3 py-4 text-center text-sm text-muted-foreground"
              :disabled="!canEditContent"
              @click="canEditContent && fileInput?.click()"
              @dragover.prevent
              @drop.prevent="onDrop"
            >
              <img
                v-if="previewSrc"
                :src="previewSrc"
                alt="Aperçu de la figurine"
                class="max-h-[min(28rem,55vh)] w-full object-contain"
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
              class="self-start"
              @click="forgetPhoto"
            >
              Retirer la photo
            </Button>
          </div>
        </form>
      </CardContent>
    </Card>
  </Teleport>

  <div class="flex min-h-0 flex-1 flex-col gap-4">
    <Alert v-if="loadError" variant="destructive" class="neon-panel-accent shrink-0">
      <AlertTitle>Chargement impossible</AlertTitle>
      <AlertDescription>{{ loadError }}</AlertDescription>
    </Alert>

    <p v-else-if="loading" class="shrink-0 text-sm text-muted-foreground">Chargement...</p>

    <p v-else-if="!figurines.length" class="shrink-0 text-sm text-muted-foreground">
      {{
        incompleteOnly
          ? 'Aucune figurine avec informations manquantes.'
          : search.trim() || painterFilter.trim()
            ? 'Aucune figurine ne correspond.'
            : armyId
              ? 'Aucune figurine pour cette sectorielle.'
              : 'Aucune figurine.'
      }}
    </p>

    <div v-else ref="scrollEl" class="min-h-0 flex-1 overflow-y-auto">
      <div class="grid grid-cols-2 items-start gap-3 lg:grid-cols-4">
      <article
        v-for="figurine in figurines"
        :key="figurine.id"
        class="neon-panel min-w-0 overflow-hidden"
      >
        <div class="relative">
          <button type="button" class="block w-full" @click="openEdit(figurine)">
            <img
              v-if="figurine.photo_url"
              :src="photoSrc(figurine)"
              :alt="figurine.nom"
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
            <div class="flex min-w-0 items-center gap-1.5">
              <p class="min-w-0 truncate font-medium" :title="figurine.nom">{{ figurine.nom }}</p>
              <Badge
                v-if="figurine.tts_loadout_id == null"
                variant="destructive"
                title="Non liée au catalogue de profils"
              >
                Orpheline
              </Badge>
            </div>
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
      <div ref="loadMoreSentinel" class="flex h-10 items-center justify-center">
        <p v-if="loadingMore" class="text-sm text-muted-foreground">Chargement…</p>
        <p v-else-if="!hasMore && total > PAGE_SIZE" class="text-sm text-muted-foreground">
          {{ total }} figurine{{ total > 1 ? 's' : '' }}
        </p>
      </div>
    </div>
  </div>
</template>
