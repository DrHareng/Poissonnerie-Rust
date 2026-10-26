<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { ChevronLeft, ChevronRight, Plus, X, ZoomIn } from '@lucide/vue'
import { toast } from 'vue-sonner'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import ColorPicker from '@/components/ColorPicker.vue'
import ImageViewer, { type ImageViewerItem } from '@/components/ImageViewer.vue'
import PlayerPicker from '@/components/PlayerPicker.vue'
import SectorialPicker from '@/components/SectorialPicker.vue'
import { useAppSidePanel } from '@/composables/useAppSidePanel'
import { useArmies } from '@/composables/useArmies'
import { useAuth } from '@/composables/useAuth'
import {
  buildTtsListBag,
  expandTtsList,
  fetchMyTtsFigurinePicks,
  fetchTtsFigurineLabels,
  fetchTtsFigurines,
  fetchTtsProfiles,
  upsertMyTtsFigurinePick,
} from '@/lib/api'
import { isListableArmy } from '@/lib/army'
import { normalizeArmyListCode } from '@/lib/armyList'
import { withBase } from '@/lib/basePath'
import { downloadBlob } from '@/lib/scenarioDocument'
import type {
  TtsFigurine,
  TtsFigurineLabel,
  TtsFigurinePickPref,
  TtsListExpandEntry,
  TtsListExpandResult,
  TtsListRankedFigurine,
  TtsProfileSummary,
} from '@/types/elo'

defineProps<{
  painterOptions: { value: string; label: string }[]
}>()

const emit = defineEmits<{
  'update:armyId': [value: string]
}>()

const { setCustomSide } = useAppSidePanel()
const { armies, ensureLoaded: ensureArmies } = useArmies()
const { isAuthenticated } = useAuth()

const code = ref('')
const preferredPainter = ref('')
const loading = ref(false)
const error = ref('')
const result = ref<TtsListExpandResult | null>(null)
/** index d'entrée → figurine sélectionnée (suggestion ou catalogue) */
const picks = ref<Record<number, TtsFigurine | TtsListRankedFigurine>>({})
/** index d'entrée Holomask → profil de couverture choisi */
const holomaskProfiles = ref<Record<number, TtsProfileSummary>>({})
/** Profils de la sectorielle (pour le choix Holomask). */
const sectorProfiles = ref<TtsProfileSummary[]>([])
/** Préférences utilisateur : clé `String(tts_loadout_id)` → figurine_id */
const pickPrefs = ref<Record<string, number>>({})
const pickPrefsLoaded = ref(false)

const bagName = ref('')
const colorG1 = ref('FFFFFF')
const colorG2 = ref('FFFFFF')
const downloading = ref(false)

const catalog = ref<TtsFigurine[]>([])
const catalogLoaded = ref(false)
const pickerOpen = ref(false)
const pickerEntryIndex = ref<number | null>(null)
const pickerArmyId = ref('')
const pickerPainter = ref('')
const pickerSearch = ref('')
const pickerLabels = ref<TtsFigurineLabel[]>([])
const pickerLoading = ref(false)
const pickerError = ref('')

const zoomOpen = ref(false)
const zoomEntryIndex = ref<number | null>(null)
const zoomIndex = ref(0)

const unresolvedCount = computed(
  () =>
    result.value?.entries.filter((entry) => !entry.holomask && !entry.profile).length ?? 0,
)

const missingFigCount = computed(
  () =>
    result.value?.entries.filter((entry) => {
      if (entry.holomask) {
        return !holomaskProfiles.value[entry.index] || !picks.value[entry.index]
      }
      return Boolean(entry.profile && !picks.value[entry.index])
    }).length ?? 0,
)

const readyPickCount = computed(() => {
  if (!result.value) return 0
  return result.value.entries.filter((entry) => {
    if (entry.holomask) {
      return Boolean(holomaskProfiles.value[entry.index] && picks.value[entry.index])
    }
    return Boolean(entry.profile && picks.value[entry.index])
  }).length
})

const pickerArmies = computed(() => armies.value.filter((army) => isListableArmy(army)))

const pickerPainterOptions = computed(() =>
  uniqueSorted(catalog.value.map((figurine) => figurine.painter)),
)

const pickerEntry = computed(() => {
  const index = pickerEntryIndex.value
  if (index == null || !result.value) return null
  return result.value.entries.find((entry) => entry.index === index) ?? null
})

/** Seule la taille compte pour le choix d’une autre fig (pas le type camo/imp). */
const requiredSilhouetteSize = computed(() => {
  const entry = pickerEntry.value
  if (!entry) return ''
  if (entry.holomask) return entry.holomask_silhouette_size?.trim() || ''
  return effectiveProfile(entry)?.silhouette_size?.trim() || ''
})

const pickerFiltered = computed(() => {
  const requiredSize = requiredSilhouetteSize.value
  if (!requiredSize) return [] as TtsFigurine[]

  const armyId = Number(pickerArmyId.value) || 0
  const painter = pickerPainter.value.trim().toLowerCase()
  const needle = pickerSearch.value.trim().toLowerCase()
  const knownIscs = armyId
    ? new Set(pickerLabels.value.map((label) => label.nom.trim().toLowerCase()))
    : null

  let rows = catalog.value
  if (knownIscs) {
    rows = rows.filter((figurine) => knownIscs.has(figurine.nom.trim().toLowerCase()))
  }
  if (painter) {
    rows = rows.filter((figurine) => figurine.painter.trim().toLowerCase() === painter)
  }
  rows = rows.filter((figurine) => figurineSilhouetteSize(figurine) === requiredSize)
  if (needle) {
    rows = rows.filter((figurine) =>
      [figurine.nom, figurine.loadout, figurine.painter, figurine.asset_bundle_url]
        .join(' ')
        .toLowerCase()
        .includes(needle),
    )
  }
  return rows
})

const sidePanelOpen = computed(() => result.value != null)

watch(sidePanelOpen, (open) => setCustomSide(open), { immediate: true })

watch(pickerArmyId, () => {
  if (pickerOpen.value) void loadPickerLabels()
})

onBeforeUnmount(() => setCustomSide(false))

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

function silhouetteOf(
  profile: Pick<TtsFigurineLabel | TtsProfileSummary, 'marker_name' | 'silhouette_size' | 'silhouette_type'>,
) {
  const marker = profile.marker_name?.trim()
  if (marker) return marker
  const size = profile.silhouette_size?.trim()
  if (!size) return ''
  const kind = profile.silhouette_type?.trim()
  if (kind && kind.toLowerCase() !== 'normale') return `S${size} · ${kind}`
  return `S${size}`
}

function figurineSilhouetteSize(figurine: TtsFigurine) {
  const isc = figurine.nom.trim().toLowerCase()
  const loadout = figurine.loadout.trim().toLowerCase()
  const sameIsc = pickerLabels.value.filter(
    (label) => label.nom.trim().toLowerCase() === isc && label.silhouette_size?.trim(),
  )
  const exact = sameIsc.find((label) => label.loadout.trim().toLowerCase() === loadout)
  return (exact ?? sameIsc[0])?.silhouette_size?.trim() ?? ''
}

function photoSrc(figurine: Pick<TtsFigurine, 'photo_url' | 'updated_at' | 'nom'>) {
  if (!figurine.photo_url) return ''
  return withBase(`${figurine.photo_url}?thumb=1&v=${figurine.updated_at}`)
}

function fullPhotoSrc(figurine: Pick<TtsFigurine, 'photo_url' | 'updated_at'>) {
  if (!figurine.photo_url) return ''
  return withBase(`${figurine.photo_url}?v=${figurine.updated_at}`)
}

function effectiveProfile(entry: TtsListExpandEntry) {
  if (entry.holomask) return holomaskProfiles.value[entry.index] ?? null
  return entry.profile
}

function pickFor(entry: TtsListExpandEntry) {
  return picks.value[entry.index] ?? null
}

function pickExactLoadout(entry: TtsListExpandEntry) {
  const pick = pickFor(entry)
  return Boolean(pick && 'exact_loadout' in pick && pick.exact_loadout)
}

function rankCatalogFigurines(isc: string, loadout: string): TtsListRankedFigurine[] {
  const iscKey = isc.trim().toLowerCase()
  const loadoutKey = loadout.trim().toLowerCase()
  const painterKey = preferredPainter.value.trim().toLowerCase()
  const ranked = catalog.value
    .filter((figurine) => figurine.nom.trim().toLowerCase() === iscKey)
    .map((figurine) => {
      const exact_loadout =
        Boolean(loadoutKey) && figurine.loadout.trim().toLowerCase() === loadoutKey
      let score = 0
      if (exact_loadout) score += 100
      if (painterKey && figurine.painter.trim().toLowerCase() === painterKey) score += 10
      if (figurine.asset_bundle_url.trim()) score += 1
      return { ...figurine, score, exact_loadout }
    })
  ranked.sort(
    (left, right) =>
      right.score - left.score ||
      left.painter.localeCompare(right.painter, 'fr', { sensitivity: 'base' }) ||
      left.id - right.id,
  )
  return ranked
}

function prefKey(ttsLoadoutId: number) {
  return String(ttsLoadoutId)
}

function rememberedFigurine(ttsLoadoutId: number): TtsFigurine | TtsListRankedFigurine | null {
  const id = pickPrefs.value[prefKey(ttsLoadoutId)]
  if (id == null) return null
  const fromCatalog = catalog.value.find((figurine) => figurine.id === id)
  if (fromCatalog) return fromCatalog
  return null
}

function choiceFigurines(entry: TtsListExpandEntry) {
  const selected = picks.value[entry.index]
  const profile = effectiveProfile(entry)
  const rows = entry.holomask
    ? (() => {
        return profile ? rankCatalogFigurines(profile.nom, profile.loadout) : []
      })()
    : [...entry.figurines]
  if (profile) {
    const remembered = rememberedFigurine(profile.tts_loadout_id)
    if (remembered && !rows.some((figurine) => figurine.id === remembered.id)) {
      rows.unshift({
        ...remembered,
        score: 10_000,
        exact_loadout: false,
      })
    } else if (remembered) {
      const index = rows.findIndex((figurine) => figurine.id === remembered.id)
      if (index > 0) {
        const [row] = rows.splice(index, 1)
        rows.unshift(row)
      }
    }
  }
  if (selected && !rows.some((figurine) => figurine.id === selected.id)) {
    rows.unshift(selected as TtsListRankedFigurine)
  }
  return rows
}

function selectFigurine(entryIndex: number, figurine: TtsFigurine | TtsListRankedFigurine) {
  picks.value = { ...picks.value, [entryIndex]: figurine }
  void persistFigurinePick(entryIndex, figurine)
}

async function persistFigurinePick(
  entryIndex: number,
  figurine: TtsFigurine | TtsListRankedFigurine,
) {
  if (!isAuthenticated.value || !result.value) return
  const entry = result.value.entries.find((row) => row.index === entryIndex)
  if (!entry) return
  const profile = effectiveProfile(entry)
  if (!profile) return
  try {
    const saved = await upsertMyTtsFigurinePick({
      tts_loadout_id: profile.tts_loadout_id,
      figurine_id: figurine.id,
    })
    pickPrefs.value = {
      ...pickPrefs.value,
      [prefKey(saved.tts_loadout_id)]: saved.figurine_id,
    }
  } catch {
    // Préférence best-effort : ne bloque pas le choix local.
  }
}

async function ensurePickPrefs() {
  if (!isAuthenticated.value) {
    pickPrefs.value = {}
    pickPrefsLoaded.value = true
    return
  }
  if (pickPrefsLoaded.value) return
  try {
    const rows: TtsFigurinePickPref[] = await fetchMyTtsFigurinePicks()
    const next: Record<string, number> = {}
    for (const row of rows) {
      next[prefKey(row.tts_loadout_id)] = row.figurine_id
    }
    pickPrefs.value = next
  } catch {
    pickPrefs.value = {}
  } finally {
    pickPrefsLoaded.value = true
  }
}

watch(isAuthenticated, () => {
  pickPrefsLoaded.value = false
  pickPrefs.value = {}
})

function resolveInitialPick(
  entry: TtsListExpandEntry,
): TtsFigurine | TtsListRankedFigurine | undefined {
  const profile = entry.profile
  if (profile) {
    const remembered = rememberedFigurine(profile.tts_loadout_id)
    if (remembered) return remembered
    const prefId = pickPrefs.value[prefKey(profile.tts_loadout_id)]
    if (prefId != null) {
      const fromEntry = entry.figurines.find((figurine) => figurine.id === prefId)
      if (fromEntry) return fromEntry
    }
  }
  return entry.figurines[0]
}

function holomaskProfileOptions(entry: TtsListExpandEntry) {
  const size = entry.holomask_silhouette_size?.trim()
  if (!size) return [] as { value: string; label: string }[]
  return sectorProfiles.value
    .filter((profile) => profile.silhouette_size?.trim() === size)
    .map((profile) => ({
      value: String(profile.id),
      label: `${profile.nom} · ${profile.loadout || '—'}`,
    }))
}

async function selectHolomaskProfile(entryIndex: number, profileId: string | undefined) {
  if (!profileId) {
    const next = { ...holomaskProfiles.value }
    delete next[entryIndex]
    holomaskProfiles.value = next
    const nextPicks = { ...picks.value }
    delete nextPicks[entryIndex]
    picks.value = nextPicks
    return
  }
  const profile = sectorProfiles.value.find((row) => String(row.id) === profileId)
  if (!profile) return
  holomaskProfiles.value = { ...holomaskProfiles.value, [entryIndex]: profile }
  await ensureCatalog()
  const first = rankCatalogFigurines(profile.nom, profile.loadout)[0]
  if (first) selectFigurine(entryIndex, first)
  else {
    const nextPicks = { ...picks.value }
    delete nextPicks[entryIndex]
    picks.value = nextPicks
  }
}

function scrollChoices(entryIndex: number, direction: -1 | 1) {
  const rail = document.querySelector<HTMLElement>(`[data-choice-rail="${entryIndex}"]`)
  if (!rail) return
  const card = rail.querySelector<HTMLElement>(':scope > *')
  const step = (card?.offsetWidth ?? 112) + 8
  rail.scrollBy({ left: direction * step * 2, behavior: 'smooth' })
}

const zoomEntry = computed(() => {
  const index = zoomEntryIndex.value
  if (index == null || !result.value) return null
  return result.value.entries.find((entry) => entry.index === index) ?? null
})

const zoomItems = computed((): ImageViewerItem[] => {
  const entry = zoomEntry.value
  if (!entry) return []
  return choiceFigurines(entry).flatMap((figurine) => {
    const src = fullPhotoSrc(figurine)
    if (!src) return []
    return [
      {
        src,
        alt: figurine.nom,
        caption: `${figurine.painter || 'Sans peintre'} · ${figurine.loadout || '—'}`,
      },
    ]
  })
})

const zoomFigurines = computed(() => {
  const entry = zoomEntry.value
  if (!entry) return [] as Array<TtsFigurine | TtsListRankedFigurine>
  return choiceFigurines(entry).filter((figurine) => Boolean(fullPhotoSrc(figurine)))
})

function openZoom(entry: TtsListExpandEntry, figurineId: number) {
  const withPhotos = choiceFigurines(entry).filter((figurine) => Boolean(fullPhotoSrc(figurine)))
  const index = withPhotos.findIndex((figurine) => figurine.id === figurineId)
  if (index < 0) return
  zoomEntryIndex.value = entry.index
  zoomIndex.value = index
  zoomOpen.value = true
}

function onZoomConfirm(index: number) {
  const entryIndex = zoomEntryIndex.value
  const figurine = zoomFigurines.value[index]
  if (entryIndex == null || !figurine) return
  selectFigurine(entryIndex, figurine)
  zoomOpen.value = false
  zoomEntryIndex.value = null
}

function onZoomCancel() {
  zoomOpen.value = false
  zoomEntryIndex.value = null
}

function bagFilename(name: string) {
  const safe = name.replace(/[\\/:*?"<>|]/g, ' ').replace(/\s+/g, ' ').trim()
  return `${safe || 'liste-tts'}.json`
}

async function ensureCatalog() {
  if (catalogLoaded.value) return
  catalog.value = (await fetchTtsFigurines({ all: true })).items
  catalogLoaded.value = true
}

async function loadPickerLabels() {
  const armyId = Number(pickerArmyId.value) || Number(result.value?.army_id) || 0
  if (!armyId) {
    pickerLabels.value = []
    return
  }
  pickerLoading.value = true
  pickerError.value = ''
  try {
    pickerLabels.value = await fetchTtsFigurineLabels(armyId)
  } catch (err) {
    pickerLabels.value = []
    pickerError.value = err instanceof Error ? err.message : 'Impossible de charger les silhouettes'
  } finally {
    pickerLoading.value = false
  }
}

async function openPicker(entry: TtsListExpandEntry) {
  if (!result.value) return
  const profile = effectiveProfile(entry)
  if (!profile) {
    toast.error(entry.holomask ? 'Choisis d’abord un profil de couverture' : 'Profil manquant')
    return
  }
  const size = entry.holomask
    ? entry.holomask_silhouette_size?.trim()
    : profile.silhouette_size?.trim()
  if (!size) {
    toast.error('Ce profil n’a pas de taille de silhouette définie')
    return
  }
  pickerEntryIndex.value = entry.index
  pickerArmyId.value = String(result.value.army_id)
  pickerPainter.value = preferredPainter.value
  pickerSearch.value = ''
  pickerOpen.value = true
  pickerError.value = ''
  try {
    await ensureArmies()
    await ensureCatalog()
    await loadPickerLabels()
  } catch (err) {
    pickerError.value = err instanceof Error ? err.message : 'Impossible de charger le catalogue'
  }
}

function closePicker() {
  pickerOpen.value = false
  pickerEntryIndex.value = null
}

function onPickerBackdrop(event: MouseEvent) {
  if (event.target === event.currentTarget) closePicker()
}

function chooseFromCatalog(figurine: TtsFigurine) {
  const index = pickerEntryIndex.value
  if (index == null) return
  selectFigurine(index, figurine)
  closePicker()
}

async function downloadBag() {
  if (!result.value || downloading.value) return
  const bagPicks = result.value.entries.flatMap((entry) => {
    const profile = effectiveProfile(entry)
    const figurine = picks.value[entry.index]
    if (!profile || !figurine) return []
    return [
      {
        figurine_id: figurine.id,
        profile_id: profile.id,
        form_id: entry.holomask ? undefined : entry.form_id ?? undefined,
        combat_group: entry.combat_group,
      },
    ]
  })
  if (!bagPicks.length) {
    toast.error('Sélectionne au moins une figurine avec un profil')
    return
  }
  const incompleteHolomask = result.value.entries.some(
    (entry) => entry.holomask && (!holomaskProfiles.value[entry.index] || !picks.value[entry.index]),
  )
  if (incompleteHolomask) {
    toast.error('Choisis un profil et une figurine pour chaque Holomask')
    return
  }
  downloading.value = true
  try {
    const object = await buildTtsListBag({
      bag_name: bagName.value.trim() || result.value.name || 'Liste TTS',
      color_g1: colorG1.value,
      color_g2: colorG2.value,
      army_id: result.value.army_id,
      picks: bagPicks,
    })
    downloadBlob(
      new Blob([JSON.stringify(object, null, 2)], { type: 'application/json' }),
      bagFilename(bagName.value || result.value.name || 'liste-tts'),
    )
    toast.success('Sac TTS téléchargé')
  } catch (err) {
    toast.error(err instanceof Error ? err.message : 'Impossible de télécharger le sac')
  } finally {
    downloading.value = false
  }
}

async function launch() {
  const normalized = normalizeArmyListCode(code.value)
  if (!normalized) {
    error.value = 'Colle un code ou une URL Army.'
    return
  }
  loading.value = true
  error.value = ''
  result.value = null
  picks.value = {}
  holomaskProfiles.value = {}
  sectorProfiles.value = []
  try {
    const expanded = await expandTtsList(normalized, preferredPainter.value || undefined)
    result.value = expanded
    bagName.value = expanded.name.trim() || expanded.sectoral_name
    colorG1.value = (expanded.default_color || 'FFFFFF').toUpperCase()
    colorG2.value = 'FFFFFF'
    emit('update:armyId', String(expanded.army_id))
    await Promise.all([ensurePickPrefs(), ensureCatalog()])
    const next: Record<number, TtsFigurine | TtsListRankedFigurine> = {}
    for (const entry of expanded.entries) {
      const first = resolveInitialPick(entry)
      if (first) next[entry.index] = first
    }
    picks.value = next
    if (expanded.entries.some((entry) => entry.holomask)) {
      sectorProfiles.value = await fetchTtsProfiles({
        armyId: expanded.army_id,
        includeFaction: false,
      })
    }
    if (expanded.entries.length === 0) {
      toast.message('Liste vide')
    } else {
      toast.success(
        `${expanded.entries.length} entrée${expanded.entries.length > 1 ? 's' : ''} décodée${expanded.entries.length > 1 ? 's' : ''}`,
      )
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Impossible de décoder la liste'
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="space-y-4">
    <div class="grid gap-3 sm:grid-cols-[1fr_minmax(10rem,14rem)_auto] sm:items-end">
      <div class="grid gap-1.5">
        <Label for="tts-list-code">Code Army</Label>
        <textarea
          id="tts-list-code"
          v-model="code"
          rows="3"
          class="border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:ring-ring flex w-full rounded-md border px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2"
          placeholder="Colle un code ou une URL infinitytheuniverse.com/army/list/…"
          autocomplete="off"
          spellcheck="false"
        />
      </div>
      <div class="grid gap-1.5">
        <Label>Peintre préféré</Label>
        <PlayerPicker
          :model-value="preferredPainter || undefined"
          :options="painterOptions"
          allow-custom
          allow-empty
          empty-label="Aucun"
          placeholder="Optionnel"
          empty-message="Aucun peintre connu."
          @update:model-value="preferredPainter = $event ?? ''"
        />
      </div>
      <Button type="button" :disabled="loading" @click="launch">
        {{ loading ? 'Décodage…' : 'Lancer' }}
      </Button>
    </div>

    <Alert v-if="error" variant="destructive">
      <AlertTitle>Erreur</AlertTitle>
      <AlertDescription>{{ error }}</AlertDescription>
    </Alert>

    <template v-if="result">
      <div class="space-y-1">
        <h2 class="text-lg font-medium">
          {{ result.sectoral_name }}
          <span v-if="result.name" class="text-muted-foreground font-normal">— {{ result.name }}</span>
        </h2>
        <p class="text-muted-foreground text-sm">
          {{ result.max_points }} pts · {{ result.entries.length }} figurine{{ result.entries.length > 1 ? 's' : '' }}
          <span v-if="unresolvedCount"> · {{ unresolvedCount }} profil{{ unresolvedCount > 1 ? 's' : '' }} non trouvé{{ unresolvedCount > 1 ? 's' : '' }}</span>
          <span v-if="missingFigCount"> · {{ missingFigCount }} sans fig 3D</span>
        </p>
      </div>

      <ul class="space-y-3">
        <li
          v-for="entry in result.entries"
          :key="entry.index"
          class="border-border rounded-lg border p-3"
        >
          <div class="flex flex-wrap items-baseline justify-between gap-2">
            <div>
              <p class="font-medium">
                <template v-if="entry.holomask">
                  Holomask
                  <span class="text-muted-foreground font-normal">
                    · pour {{ entry.holomask_source_nom || '—' }}
                  </span>
                </template>
                <template v-else-if="entry.profile">
                  {{ entry.profile.nom }}
                  <span class="text-muted-foreground font-normal">· {{ entry.profile.loadout || '—' }}</span>
                </template>
                <template v-else>
                  Unité {{ entry.unit_id }}
                  <span class="text-muted-foreground font-normal">
                    · g{{ entry.group_id }} / opt {{ entry.option_id }}
                  </span>
                </template>
              </p>
              <p class="text-muted-foreground text-xs">
                CG {{ entry.combat_group }}
                <span v-if="entry.holomask">
                  · couverture S{{ entry.holomask_silhouette_size || '—' }}
                </span>
                <span v-if="entry.form_id"> · périphérique</span>
                <span v-if="entry.reinforcements"> · renforts</span>
                <span v-if="entry.customization_json"> · custom</span>
                <span v-if="!entry.holomask && entry.profile && silhouetteOf(entry.profile)">
                  · {{ silhouetteOf(entry.profile) }}
                </span>
                <span v-if="!entry.holomask && !entry.profile" class="text-destructive"> · profil absent</span>
              </p>
            </div>
            <p v-if="pickFor(entry)" class="text-muted-foreground text-xs">
              {{ pickFor(entry)?.painter || 'Sans peintre' }}
              <span v-if="pickExactLoadout(entry)"> · loadout exact</span>
            </p>
          </div>

          <div v-if="entry.holomask" class="mt-3 grid gap-1.5 sm:max-w-md">
            <Label>Profil de couverture</Label>
            <PlayerPicker
              :model-value="holomaskProfiles[entry.index] ? String(holomaskProfiles[entry.index].id) : undefined"
              :options="holomaskProfileOptions(entry)"
              allow-empty
              empty-label="Choisir un profil…"
              placeholder="Même sectorielle · même silhouette"
              empty-message="Aucun profil de cette taille dans la sectorielle."
              @update:model-value="selectHolomaskProfile(entry.index, $event)"
            />
          </div>

          <div class="mt-3 flex items-stretch gap-1">
            <Button
              type="button"
              variant="outline"
              size="icon"
              class="h-auto min-h-28 shrink-0 self-stretch"
              aria-label="Figurines précédentes"
              @click="scrollChoices(entry.index, -1)"
            >
              <ChevronLeft class="size-5" />
            </Button>

            <div
              :data-choice-rail="entry.index"
              class="flex min-w-0 flex-1 gap-2 overflow-x-auto overscroll-x-contain scroll-smooth [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
            >
              <button
                v-for="figurine in choiceFigurines(entry)"
                :key="figurine.id"
                type="button"
                class="border-border hover:border-foreground/40 focus-visible:ring-ring flex w-28 shrink-0 flex-col overflow-hidden rounded-md border text-left transition focus-visible:outline-none focus-visible:ring-2"
                :class="pickFor(entry)?.id === figurine.id ? 'border-foreground ring-1 ring-foreground' : ''"
                @click="selectFigurine(entry.index, figurine)"
              >
                <div class="bg-muted relative aspect-square w-full overflow-hidden">
                  <img
                    v-if="photoSrc(figurine)"
                    :src="photoSrc(figurine)"
                    :alt="figurine.nom"
                    class="size-full object-contain"
                  >
                  <div
                    v-else
                    class="text-muted-foreground flex size-full items-center justify-center text-xs"
                  >
                    Pas de photo
                  </div>
                  <Button
                    v-if="fullPhotoSrc(figurine)"
                    type="button"
                    variant="secondary"
                    size="icon-sm"
                    class="absolute top-1 right-1 size-7 opacity-90"
                    aria-label="Agrandir la photo"
                    @click.stop="openZoom(entry, figurine.id)"
                  >
                    <ZoomIn class="size-3.5" />
                  </Button>
                </div>
                <div class="space-y-0.5 p-1.5">
                  <p class="truncate text-xs font-medium">{{ figurine.painter || '—' }}</p>
                  <p class="text-muted-foreground truncate text-[11px]">{{ figurine.loadout || '—' }}</p>
                </div>
              </button>

              <button
                v-if="effectiveProfile(entry)"
                type="button"
                class="border-border hover:border-foreground/40 focus-visible:ring-ring text-muted-foreground hover:text-foreground flex w-28 shrink-0 flex-col items-center justify-center gap-1 rounded-md border border-dashed px-2 py-3 text-center transition focus-visible:outline-none focus-visible:ring-2"
                @click="openPicker(entry)"
              >
                <Plus class="size-5 shrink-0" />
                <span class="text-[11px] leading-tight">Choisir une autre figurine</span>
              </button>
            </div>

            <Button
              type="button"
              variant="outline"
              size="icon"
              class="h-auto min-h-28 shrink-0 self-stretch"
              aria-label="Figurines suivantes"
              @click="scrollChoices(entry.index, 1)"
            >
              <ChevronRight class="size-5" />
            </Button>
          </div>

          <p
            v-if="entry.holomask && !holomaskProfiles[entry.index]"
            class="text-muted-foreground mt-2 text-sm"
          >
            Choisis un profil de la sectorielle avec la silhouette S{{ entry.holomask_silhouette_size || '—' }}.
          </p>
          <p
            v-else-if="effectiveProfile(entry) && !choiceFigurines(entry).length"
            class="text-muted-foreground mt-2 text-sm"
          >
            Aucune figurine suggérée pour ce nom — utilise « Choisir une autre figurine ».
          </p>
        </li>
      </ul>
    </template>

    <Teleport defer to="#app-side-panel">
      <Card
        v-if="result"
        class="neon-panel flex h-full max-h-full min-h-0 flex-col overflow-hidden"
      >
        <CardHeader class="shrink-0 pb-3">
          <CardTitle>Sac TTS</CardTitle>
          <CardDescription>
            {{ readyPickCount }} / {{ result.entries.length }} figurine{{ result.entries.length > 1 ? 's' : '' }} prête{{ readyPickCount > 1 ? 's' : '' }}
          </CardDescription>
        </CardHeader>
        <CardContent class="min-h-0 flex-1 space-y-4 overflow-y-auto pt-0">
          <div class="space-y-1.5">
            <Label for="tts-bag-name">Nom du sac</Label>
            <Input
              id="tts-bag-name"
              v-model="bagName"
              type="text"
              autocomplete="off"
              spellcheck="false"
            />
          </div>
          <div class="space-y-2">
            <p class="text-sm font-medium">Couleur G1</p>
            <ColorPicker v-model="colorG1" />
          </div>
          <div class="space-y-2">
            <p class="text-sm font-medium">Couleur G2</p>
            <ColorPicker v-model="colorG2" />
          </div>
          <Button
            type="button"
            class="w-full"
            :disabled="downloading || readyPickCount === 0"
            @click="downloadBag"
          >
            {{ downloading ? 'Génération…' : 'Télécharger le sac' }}
          </Button>
        </CardContent>
      </Card>
    </Teleport>

    <ImageViewer
      v-model:open="zoomOpen"
      v-model:index="zoomIndex"
      :items="zoomItems"
      confirmable
      @confirm="onZoomConfirm"
      @cancel="onZoomCancel"
    />

    <Teleport to="body">
      <div
        v-if="pickerOpen && pickerEntry"
        class="player-detail-overlay"
        role="dialog"
        aria-modal="true"
        aria-labelledby="tts-list-fig-picker-title"
        @click="onPickerBackdrop"
      >
        <Card class="player-detail-modal neon-panel flex max-h-[min(85vh,52rem)] w-full max-w-5xl! flex-col overflow-hidden!">
          <CardHeader class="relative shrink-0 pr-12">
            <CardTitle id="tts-list-fig-picker-title">
              Choisir une figurine
            </CardTitle>
            <CardDescription>
              {{ pickerEntry.profile?.nom }} · S{{ requiredSilhouetteSize || '—' }}
            </CardDescription>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              class="absolute top-3 right-3"
              aria-label="Fermer"
              @click="closePicker"
            >
              <X class="size-4" />
            </Button>
          </CardHeader>
          <CardContent class="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden">
            <div class="flex shrink-0 flex-wrap items-end gap-2">
              <div class="grid min-w-56 flex-1 gap-1.5 sm:max-w-md">
                <Label>Sectorielle</Label>
                <SectorialPicker
                  :model-value="pickerArmyId || undefined"
                  :armies="pickerArmies"
                  clearable
                  allow-empty
                  empty-label="Toutes"
                  placeholder="Filtrer par sectorielle"
                  @update:model-value="pickerArmyId = $event ?? ''"
                />
              </div>
              <div class="grid min-w-44 flex-1 gap-1.5 sm:max-w-xs">
                <Label>Peintre</Label>
                <PlayerPicker
                  :model-value="pickerPainter || undefined"
                  :options="pickerPainterOptions"
                  allow-custom
                  allow-empty
                  empty-label="Tous"
                  placeholder="Filtrer par peintre"
                  empty-message="Aucun peintre connu."
                  @update:model-value="pickerPainter = $event ?? ''"
                />
              </div>
              <div class="relative w-full max-w-xs">
                <Input
                  v-model="pickerSearch"
                  class="w-full pr-9"
                  placeholder="Nom, loadout, peintre…"
                  autocomplete="off"
                />
                <button
                  v-if="pickerSearch"
                  type="button"
                  class="text-muted-foreground hover:text-foreground absolute top-1/2 right-2 -translate-y-1/2 rounded-sm p-0.5"
                  title="Effacer la recherche"
                  aria-label="Effacer la recherche"
                  @click="pickerSearch = ''"
                >
                  <X class="size-4 opacity-60" />
                </button>
              </div>
            </div>

            <Alert v-if="pickerError" variant="destructive" class="shrink-0">
              <AlertTitle>Erreur</AlertTitle>
              <AlertDescription>{{ pickerError }}</AlertDescription>
            </Alert>
            <p v-else-if="pickerLoading" class="text-muted-foreground shrink-0 text-sm">
              Chargement…
            </p>
            <p
              v-else-if="!pickerFiltered.length"
              class="text-muted-foreground shrink-0 text-sm"
            >
              Aucune figurine en S{{ requiredSilhouetteSize }}.
            </p>
            <div v-else class="min-h-0 flex-1 overflow-y-auto">
              <div class="grid grid-cols-3 gap-2 sm:grid-cols-4 md:grid-cols-5">
                <button
                  v-for="figurine in pickerFiltered"
                  :key="figurine.id"
                  type="button"
                  class="border-border hover:border-foreground/40 focus-visible:ring-ring flex flex-col overflow-hidden rounded-md border text-left transition focus-visible:outline-none focus-visible:ring-2"
                  @click="chooseFromCatalog(figurine)"
                >
                  <div class="bg-muted aspect-square w-full overflow-hidden">
                    <img
                      v-if="photoSrc(figurine)"
                      :src="photoSrc(figurine)"
                      :alt="figurine.nom"
                      class="size-full object-cover"
                    >
                    <div
                      v-else
                      class="text-muted-foreground flex size-full items-center justify-center text-xs"
                    >
                      Pas de photo
                    </div>
                  </div>
                  <div class="space-y-0.5 p-1.5">
                    <p class="truncate text-xs font-medium">{{ figurine.nom }}</p>
                    <p class="text-muted-foreground truncate text-[11px]">
                      {{ figurine.painter || '—' }} · {{ figurine.loadout || '—' }}
                    </p>
                  </div>
                </button>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </Teleport>
  </div>
</template>
