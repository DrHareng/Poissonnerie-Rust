<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import {
  assignAdminFigurineLoadout,
  assignAdminUnitProfile,
  createAdminLoadout,
  createAdminTts,
  deleteAdminLoadout,
  deleteAdminTts,
  fetchAdminUnit,
  fetchAdminUnits,
  updateAdminTts,
  type AdminFigurine,
  type AdminLoadoutChoice,
  type AdminTtsChoice,
  type AdminUnitDetail,
  type AdminUnitProfile,
  type AdminUnitSummary,
} from '@/lib/api'
import { isListableArmy } from '@/lib/army'
import { pageTitle } from '@/lib/pageTitle'
import { adminTabs } from '@/lib/pageTitleTabs'
import { useAppSidePanel } from '@/composables/useAppSidePanel'
import { useArmies } from '@/composables/useArmies'
import { useAuth } from '@/composables/useAuth'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import SectorialPicker from '@/components/SectorialPicker.vue'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

type UnitTab = 'profils' | 'tts' | 'loadouts'

type TtsDraft = {
  nom_tts: string
  description_tts: string
}

const { isAdmin, loading: authLoading, initialized } = useAuth()
const { setCustomSide } = useAppSidePanel()
const { armies, ensureLoaded: ensureArmiesLoaded } = useArmies()
const showUnitSide = computed(() => initialized.value && isAdmin.value)

watch(showUnitSide, (active) => setCustomSide(active), { immediate: true })

const UNIT_PAGE_SIZE = 40

const query = ref('')
const armyFilter = ref<string | undefined>(undefined)
const units = ref<AdminUnitSummary[]>([])
const unitsTotal = ref(0)
const unitsLoading = ref(false)
const unitsLoadingMore = ref(false)
const unitsScrollEl = ref<HTMLElement | null>(null)
const unitsLoadMoreSentinel = ref<HTMLElement | null>(null)
const narrowUnitsScrollEl = ref<HTMLElement | null>(null)
const narrowUnitsLoadMoreSentinel = ref<HTMLElement | null>(null)
let unitsLoadGeneration = 0
let unitsSearchTimer: ReturnType<typeof setTimeout> | null = null
let unitsLoadMoreObserver: IntersectionObserver | null = null
let narrowUnitsLoadMoreObserver: IntersectionObserver | null = null

const pickerArmies = computed(() => armies.value.filter((army) => isListableArmy(army)))
const selectedArmyId = computed(() => {
  const raw = armyFilter.value
  if (!raw) return null
  const id = Number(raw)
  return Number.isFinite(id) && id > 0 ? id : null
})
const selectedId = ref<number | null>(null)
const detail = ref<AdminUnitDetail | null>(null)
const detailLoading = ref(false)
const savingKey = ref<string | null>(null)
const activeTab = ref<UnitTab>('profils')
const extraTts = ref<AdminTtsChoice[]>([])
const extraLoadouts = ref<AdminLoadoutChoice[]>([])
const newTtsName = ref('')
const newTtsDescription = ref('')
const newLoadoutName = ref('')
const creating = ref(false)
const ttsDrafts = reactive<Record<number, TtsDraft>>({})

const selectClass =
  'h-9 w-full rounded-md border border-input bg-transparent px-2 text-sm disabled:opacity-50'
const textareaClass =
  'min-h-40 w-full rounded-md border border-input bg-transparent px-3 py-2 font-mono text-xs leading-relaxed disabled:opacity-50'

const profiles = computed(() => detail.value?.profiles ?? [])

const ttsOptions = computed(() => {
  const byId = new Map<number, AdminTtsChoice>()
  for (const item of detail.value?.tts_profiles ?? []) byId.set(item.id, item)
  for (const item of extraTts.value) {
    if (!byId.has(item.id)) byId.set(item.id, item)
  }
  return [...byId.values()].sort((a, b) =>
    a.nom_tts.localeCompare(b.nom_tts, 'fr', { sensitivity: 'base' }),
  )
})

const loadoutOptions = computed(() => {
  const byId = new Map<number, AdminLoadoutChoice>()
  for (const item of detail.value?.loadouts ?? []) byId.set(item.id, item)
  for (const item of extraLoadouts.value) {
    if (!byId.has(item.id)) byId.set(item.id, item)
  }
  return [...byId.values()].sort((a, b) =>
    a.nom.localeCompare(b.nom, 'fr', { sensitivity: 'base' }),
  )
})

const ttsCatalog = computed(() => ttsOptions.value)
const loadoutCatalog = computed(() => loadoutOptions.value)

function figurinesForLoadout(loadoutId: number) {
  return (detail.value?.figurines ?? []).filter((figurine) => figurine.loadout_id === loadoutId)
}

function profileLabel(profile: AdminUnitProfile) {
  return profile.loadout.trim() || 'sans équipement'
}

function syncTtsDrafts(next: AdminUnitDetail) {
  const keep = new Set<number>()
  for (const tts of next.tts_profiles) {
    keep.add(tts.id)
    ttsDrafts[tts.id] = {
      nom_tts: tts.nom_tts,
      description_tts: tts.description_tts,
    }
  }
  for (const item of extraTts.value) {
    keep.add(item.id)
    if (!ttsDrafts[item.id]) {
      ttsDrafts[item.id] = {
        nom_tts: item.nom_tts,
        description_tts: item.description_tts,
      }
    }
  }
  for (const id of Object.keys(ttsDrafts).map(Number)) {
    if (!keep.has(id)) delete ttsDrafts[id]
  }
}

function ensureTtsDraft(ttsId: number, fallback?: { nom_tts: string; description_tts: string }) {
  if (!ttsDrafts[ttsId]) {
    ttsDrafts[ttsId] = {
      nom_tts: fallback?.nom_tts ?? '',
      description_tts: fallback?.description_tts ?? '',
    }
  }
  return ttsDrafts[ttsId]
}

function ttsDraftDirty(tts: Pick<AdminTtsChoice, 'id' | 'nom_tts' | 'description_tts'>) {
  const draft = ttsDrafts[tts.id]
  if (!draft) return false
  return (
    draft.nom_tts !== tts.nom_tts || draft.description_tts !== tts.description_tts
  )
}

function pruneExtras(next: AdminUnitDetail) {
  const ttsIds = new Set(next.tts_profiles.map((item) => item.id))
  const loadoutIds = new Set(next.loadouts.map((item) => item.id))
  extraTts.value = extraTts.value.filter((item) => !ttsIds.has(item.id))
  extraLoadouts.value = extraLoadouts.value.filter((item) => !loadoutIds.has(item.id))
}

const unitsHasMore = computed(() => units.value.length < unitsTotal.value)

function scheduleUnitsSearch() {
  if (unitsSearchTimer != null) clearTimeout(unitsSearchTimer)
  unitsSearchTimer = setTimeout(() => {
    unitsSearchTimer = null
    void loadUnits(true)
  }, 200)
}

function canLoadMoreUnits() {
  return (
    isAdmin.value &&
    !unitsLoading.value &&
    !unitsLoadingMore.value &&
    unitsHasMore.value
  )
}

function isNearUnitsBottom(root: HTMLElement | null) {
  if (!root || root.clientHeight < 8) return false
  // Pas de scroll interne = pas de pagination auto (évite de tout charger d’un coup).
  if (root.scrollHeight <= root.clientHeight + 4) return false
  return root.scrollTop + root.clientHeight >= root.scrollHeight - 120
}

function requestLoadMoreUnits() {
  if (!canLoadMoreUnits()) return
  void loadUnits(false)
}

function onUnitsScroll(event: Event) {
  const root = event.currentTarget
  if (!(root instanceof HTMLElement)) return
  if (isNearUnitsBottom(root)) requestLoadMoreUnits()
}

function bindUnitsLoadMoreObserver(
  root: HTMLElement | null,
  target: HTMLElement | null,
  current: IntersectionObserver | null,
): IntersectionObserver | null {
  current?.disconnect()
  if (
    !root ||
    !target ||
    !unitsHasMore.value ||
    root.clientHeight < 8 ||
    root.scrollHeight <= root.clientHeight + 4 ||
    typeof IntersectionObserver === 'undefined'
  ) {
    return null
  }
  const observer = new IntersectionObserver(
    (entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        requestLoadMoreUnits()
      }
    },
    { root, rootMargin: '120px 0px' },
  )
  observer.observe(target)
  return observer
}

function refreshUnitsObservers() {
  void nextTick(() => {
    unitsLoadMoreObserver = bindUnitsLoadMoreObserver(
      unitsScrollEl.value,
      unitsLoadMoreSentinel.value,
      unitsLoadMoreObserver,
    )
    narrowUnitsLoadMoreObserver = bindUnitsLoadMoreObserver(
      narrowUnitsScrollEl.value,
      narrowUnitsLoadMoreSentinel.value,
      narrowUnitsLoadMoreObserver,
    )
  })
}

async function loadUnits(reset = true) {
  if (!isAdmin.value) {
    units.value = []
    unitsTotal.value = 0
    unitsLoading.value = false
    unitsLoadingMore.value = false
    return
  }
  const generation = reset ? ++unitsLoadGeneration : unitsLoadGeneration
  if (reset) {
    unitsLoading.value = true
    units.value = []
    unitsTotal.value = 0
  } else {
    if (!canLoadMoreUnits()) return
    unitsLoadingMore.value = true
  }
  const offset = reset ? 0 : units.value.length
  const armyId = selectedArmyId.value
  try {
    const page = await fetchAdminUnits({
      query: query.value,
      armyId,
      limit: UNIT_PAGE_SIZE,
      offset,
    })
    if (generation !== unitsLoadGeneration) return
    const items = Array.isArray(page.items) ? page.items : []
    unitsTotal.value = Number(page.total) || 0
    if (reset) {
      units.value = items
    } else {
      const seen = new Set(units.value.map((unit) => unit.id))
      const fresh = items.filter((unit) => !seen.has(unit.id))
      if (fresh.length === 0) {
        unitsTotal.value = units.value.length
      } else {
        units.value = [...units.value, ...fresh]
      }
    }
  } catch (error) {
    if (generation !== unitsLoadGeneration) return
    toast.error(
      error instanceof Error ? error.message : 'Impossible de charger les unités',
    )
    if (reset) {
      units.value = []
      unitsTotal.value = 0
    }
  } finally {
    if (generation === unitsLoadGeneration) {
      unitsLoading.value = false
      unitsLoadingMore.value = false
      refreshUnitsObservers()
    }
  }
}

async function openUnit(id: number) {
  selectedId.value = id
  detailLoading.value = true
  try {
    const next = await fetchAdminUnit(id)
    detail.value = next
    pruneExtras(next)
    syncTtsDrafts(next)
  } catch (error) {
    detail.value = null
    toast.error(
      error instanceof Error ? error.message : "Impossible de charger l'unité",
    )
  } finally {
    detailLoading.value = false
  }
}

async function assignProfile(
  profile: AdminUnitProfile,
  ttsId: number,
  loadoutId: number,
) {
  if (ttsId === profile.tts_id && loadoutId === profile.loadout_id) return
  savingKey.value = `profile-${profile.id}`
  try {
    const next = await assignAdminUnitProfile(profile.id, ttsId, loadoutId)
    detail.value = next
    pruneExtras(next)
    syncTtsDrafts(next)
    toast.success('Profil mis à jour')
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible d’enregistrer le profil',
    )
    if (selectedId.value != null) await openUnit(selectedId.value)
  } finally {
    savingKey.value = null
  }
}

async function saveTts(
  ttsId: number,
  source?: { nom_tts: string; description_tts: string },
) {
  if (selectedId.value == null) return
  const draft = ensureTtsDraft(ttsId, source)
  const nom = draft.nom_tts.trim()
  if (!nom) {
    toast.error('Indiquez un nom de profil TTS')
    return
  }
  savingKey.value = `tts-save-${ttsId}`
  try {
    const next = await updateAdminTts(ttsId, selectedId.value, {
      nom_tts: nom,
      description_tts: draft.description_tts,
    })
    detail.value = next
    pruneExtras(next)
    syncTtsDrafts(next)
    toast.success('Profil TTS enregistré')
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible d’enregistrer le profil TTS',
    )
  } finally {
    savingKey.value = null
  }
}

async function addTts() {
  const nom = newTtsName.value.trim()
  if (!nom) return
  creating.value = true
  try {
    const created = await createAdminTts(nom, newTtsDescription.value.trim())
    extraTts.value = [
      ...extraTts.value.filter((item) => item.id !== created.id),
      {
        id: created.id,
        nom_tts: nom,
        description_tts: newTtsDescription.value.trim(),
        profile_count: 0,
      },
    ]
    ttsDrafts[created.id] = {
      nom_tts: nom,
      description_tts: newTtsDescription.value.trim(),
    }
    newTtsName.value = ''
    newTtsDescription.value = ''
    toast.success('Profil TTS créé — assignez-le à un profil')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Création impossible')
  } finally {
    creating.value = false
  }
}

async function addLoadout() {
  const nom = newLoadoutName.value.trim()
  if (!nom) return
  creating.value = true
  try {
    const created = await createAdminLoadout(nom)
    extraLoadouts.value = [
      ...extraLoadouts.value.filter((item) => item.id !== created.id),
      {
        id: created.id,
        nom,
        silhouette_size: null,
        silhouette_type: null,
        marker_name: null,
        profile_count: 0,
        figurine_count: 0,
      },
    ]
    newLoadoutName.value = ''
    toast.success('Loadout créé — assignez-le à un profil')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Création impossible')
  } finally {
    creating.value = false
  }
}

async function assignFigurine(figurine: AdminFigurine, loadoutId: number) {
  if (loadoutId === figurine.loadout_id) return
  savingKey.value = `figurine-${figurine.id}`
  try {
    const next = await assignAdminFigurineLoadout(figurine.id, loadoutId)
    detail.value = next
    pruneExtras(next)
    syncTtsDrafts(next)
    toast.success('Figurine rattachée au loadout')
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible d’enregistrer la figurine',
    )
    if (selectedId.value != null) await openUnit(selectedId.value)
  } finally {
    savingKey.value = null
  }
}

async function removeTts(tts: AdminTtsChoice) {
  if (selectedId.value == null || tts.profile_count > 0) return
  savingKey.value = `tts-${tts.id}`
  try {
    const next = await deleteAdminTts(tts.id, selectedId.value)
    detail.value = next
    extraTts.value = extraTts.value.filter((item) => item.id !== tts.id)
    pruneExtras(next)
    syncTtsDrafts(next)
    toast.success('Profil TTS supprimé')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Suppression impossible')
  } finally {
    savingKey.value = null
  }
}

async function removeLoadout(loadout: AdminLoadoutChoice) {
  if (
    selectedId.value == null ||
    loadout.profile_count > 0 ||
    loadout.figurine_count > 0
  ) {
    return
  }
  savingKey.value = `loadout-${loadout.id}`
  try {
    const next = await deleteAdminLoadout(loadout.id, selectedId.value)
    detail.value = next
    extraLoadouts.value = extraLoadouts.value.filter((item) => item.id !== loadout.id)
    pruneExtras(next)
    syncTtsDrafts(next)
    toast.success('Loadout supprimé')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Suppression impossible')
  } finally {
    savingKey.value = null
  }
}

onMounted(() => {
  document.title = pageTitle('Profils TTS')
  void ensureArmiesLoaded().catch(() => {
    /* picker vide si échec */
  })
})

watch(
  [initialized, isAdmin],
  ([ready, admin]) => {
    if (ready && admin) void loadUnits(true)
    if (ready && !admin) {
      units.value = []
      unitsTotal.value = 0
      unitsLoading.value = false
      unitsLoadingMore.value = false
    }
  },
  { immediate: true },
)

function onArmyFilterUpdate(value: string | undefined) {
  armyFilter.value = value
  void loadUnits(true)
}

watch(
  [unitsHasMore, unitsScrollEl, unitsLoadMoreSentinel, narrowUnitsScrollEl, narrowUnitsLoadMoreSentinel],
  () => {
    refreshUnitsObservers()
  },
)

onBeforeUnmount(() => {
  if (unitsSearchTimer != null) clearTimeout(unitsSearchTimer)
  unitsLoadMoreObserver?.disconnect()
  narrowUnitsLoadMoreObserver?.disconnect()
})
</script>

<template>
  <div class="page-stack">
    <PageTitleTabs ariaLabel="Administration" :tabs="adminTabs" />

    <div v-if="authLoading || !initialized" class="text-sm text-muted-foreground">
      Chargement...
    </div>

    <Alert v-else-if="!isAdmin" variant="destructive" class="neon-panel-accent">
      <AlertTitle>Accès réservé</AlertTitle>
      <AlertDescription>
        Cette page n’est disponible que pour les administrateurs.
      </AlertDescription>
    </Alert>

    <template v-else>
      <p class="text-sm text-muted-foreground">
        Onglets : profils (FK nom TTS / loadout), profils TTS (description éditable),
        loadouts.
      </p>

      <Teleport defer to="#app-side-panel">
        <Card
          v-if="showUnitSide"
          class="neon-panel flex h-full min-h-0 flex-col overflow-hidden"
        >
          <CardHeader class="shrink-0">
            <CardTitle>Unités</CardTitle>
          </CardHeader>
          <CardContent class="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden">
            <div class="grid shrink-0 gap-1.5">
              <Label>Sectorielle</Label>
              <SectorialPicker
                :model-value="armyFilter"
                :armies="pickerArmies"
                allow-empty
                empty-label="Toutes"
                clearable
                placeholder="Toutes les sectorielles"
                @update:model-value="onArmyFilterUpdate"
              />
            </div>
            <Input
              v-model="query"
              class="shrink-0"
              placeholder="Rechercher une unité"
              @update:model-value="scheduleUnitsSearch"
            />
            <p
              v-if="unitsLoading && units.length === 0"
              class="text-sm text-muted-foreground"
            >
              Chargement…
            </p>
            <p
              v-else-if="!unitsLoading && units.length === 0"
              class="text-sm text-muted-foreground"
            >
              Aucune unité.
            </p>
            <div
              v-else
              ref="unitsScrollEl"
              class="min-h-0 flex-1 overflow-y-auto"
              @scroll.passive="onUnitsScroll"
            >
              <ul class="grid content-start gap-0">
                <li v-for="unit in units" :key="unit.id">
                  <button
                    type="button"
                    class="flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left text-sm hover:bg-muted"
                    :class="selectedId === unit.id ? 'bg-muted font-medium' : ''"
                    @click="openUnit(unit.id)"
                  >
                    <span>{{ unit.nom }}</span>
                    <span class="text-muted-foreground">{{ unit.profile_count }}</span>
                  </button>
                </li>
                <li
                  ref="unitsLoadMoreSentinel"
                  class="flex h-10 items-center justify-center"
                >
                  <p v-if="unitsLoadingMore" class="text-xs text-muted-foreground">
                    Chargement…
                  </p>
                  <p
                    v-else-if="!unitsHasMore && unitsTotal > UNIT_PAGE_SIZE"
                    class="text-xs text-muted-foreground"
                  >
                    {{ unitsTotal }} unités
                  </p>
                </li>
              </ul>
            </div>
          </CardContent>
        </Card>
      </Teleport>

      <Card v-if="showUnitSide" class="neon-panel lg:hidden">
        <CardHeader>
          <CardTitle>Unités</CardTitle>
        </CardHeader>
        <CardContent class="grid gap-3">
          <div class="grid gap-1.5">
            <Label>Sectorielle</Label>
            <SectorialPicker
              :model-value="armyFilter"
              :armies="pickerArmies"
              allow-empty
              empty-label="Toutes"
              clearable
              placeholder="Toutes les sectorielles"
              @update:model-value="onArmyFilterUpdate"
            />
          </div>
          <Input
            v-model="query"
            placeholder="Rechercher une unité"
            @update:model-value="scheduleUnitsSearch"
          />
          <p
            v-if="unitsLoading && units.length === 0"
            class="text-sm text-muted-foreground"
          >
            Chargement…
          </p>
          <p
            v-else-if="!unitsLoading && units.length === 0"
            class="text-sm text-muted-foreground"
          >
            Aucune unité.
          </p>
          <div
            v-else
            ref="narrowUnitsScrollEl"
            class="max-h-[32rem] overflow-y-auto"
            @scroll.passive="onUnitsScroll"
          >
            <ul class="grid content-start gap-0">
              <li v-for="unit in units" :key="`narrow-${unit.id}`">
                <button
                  type="button"
                  class="flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left text-sm hover:bg-muted"
                  :class="selectedId === unit.id ? 'bg-muted font-medium' : ''"
                  @click="openUnit(unit.id)"
                >
                  <span>{{ unit.nom }}</span>
                  <span class="text-muted-foreground">{{ unit.profile_count }}</span>
                </button>
              </li>
              <li
                ref="narrowUnitsLoadMoreSentinel"
                class="flex h-10 items-center justify-center"
              >
                <p v-if="unitsLoadingMore" class="text-xs text-muted-foreground">
                  Chargement…
                </p>
                <p
                  v-else-if="!unitsHasMore && unitsTotal > UNIT_PAGE_SIZE"
                  class="text-xs text-muted-foreground"
                >
                  {{ unitsTotal }} unités
                </p>
              </li>
            </ul>
          </div>
        </CardContent>
      </Card>

      <div class="grid min-h-0 flex-1 content-start gap-4">
        <p v-if="detailLoading" class="text-sm text-muted-foreground">Chargement…</p>
        <p v-else-if="!detail" class="text-sm text-muted-foreground">
          Choisissez une unité pour voir ses profils, profils TTS et loadouts.
        </p>
        <template v-else>
          <div class="flex flex-wrap items-end justify-between gap-3">
            <h2 class="font-display text-lg font-semibold tracking-wide uppercase">
              {{ detail.unit.nom }}
            </h2>
            <nav class="pool-tabs" aria-label="Sections de l’unité">
              <button
                type="button"
                class="pool-tab"
                :class="{ 'pool-tab--active': activeTab === 'profils' }"
                @click="activeTab = 'profils'"
              >
                Profils
              </button>
              <button
                type="button"
                class="pool-tab"
                :class="{ 'pool-tab--active': activeTab === 'tts' }"
                @click="activeTab = 'tts'"
              >
                Profils TTS
              </button>
              <button
                type="button"
                class="pool-tab"
                :class="{ 'pool-tab--active': activeTab === 'loadouts' }"
                @click="activeTab = 'loadouts'"
              >
                Loadout
              </button>
            </nav>
          </div>

          <Card v-if="activeTab === 'profils'" class="neon-panel h-fit w-full">
            <CardHeader>
              <CardTitle>Profils</CardTitle>
            </CardHeader>
            <CardContent class="grid content-start gap-2">
              <p v-if="profiles.length === 0" class="text-sm text-muted-foreground">
                Aucun profil.
              </p>
              <div
                v-for="profile in profiles"
                :key="profile.id"
                class="grid gap-2 border-b border-border py-2 first:pt-0 last:border-0 last:pb-0"
              >
                <div class="flex flex-wrap items-baseline justify-between gap-2">
                  <p class="font-medium">{{ profileLabel(profile) }}</p>
                  <p class="text-xs text-muted-foreground">
                    #{{ profile.id }} · {{ profile.faction_label }}
                  </p>
                </div>
                <div class="grid gap-3 lg:grid-cols-2">
                  <div class="grid gap-1.5">
                    <Label :for="`tts-${profile.id}`">Nom TTS</Label>
                    <select
                      :id="`tts-${profile.id}`"
                      :class="selectClass"
                      :disabled="savingKey === `profile-${profile.id}`"
                      :value="profile.tts_id"
                      @change="
                        assignProfile(
                          profile,
                          Number(($event.target as HTMLSelectElement).value),
                          profile.loadout_id,
                        )
                      "
                    >
                      <option
                        v-for="tts in ttsOptions"
                        :key="tts.id"
                        :value="tts.id"
                      >
                        {{ tts.nom_tts }}
                      </option>
                    </select>
                  </div>
                  <div class="grid gap-1.5">
                    <Label :for="`loadout-${profile.id}`">Loadout</Label>
                    <select
                      :id="`loadout-${profile.id}`"
                      :class="selectClass"
                      :disabled="savingKey === `profile-${profile.id}`"
                      :value="profile.loadout_id"
                      @change="
                        assignProfile(
                          profile,
                          profile.tts_id,
                          Number(($event.target as HTMLSelectElement).value),
                        )
                      "
                    >
                      <option
                        v-for="loadout in loadoutOptions"
                        :key="loadout.id"
                        :value="loadout.id"
                      >
                        {{ loadout.nom }}
                      </option>
                    </select>
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card v-else-if="activeTab === 'tts'" class="neon-panel h-fit w-full">
            <CardHeader>
              <CardTitle>Profils TTS</CardTitle>
            </CardHeader>
            <CardContent class="grid content-start gap-6">
              <p v-if="ttsCatalog.length === 0" class="text-sm text-muted-foreground">
                Aucun profil TTS.
              </p>
              <div
                v-for="tts in ttsCatalog"
                :key="tts.id"
                class="grid gap-3 border-b border-border pb-6 last:border-0 last:pb-0"
              >
                <div class="flex flex-wrap items-center justify-between gap-2">
                  <p class="text-xs text-muted-foreground">
                    #{{ tts.id }} · {{ tts.profile_count }} profil{{
                      tts.profile_count > 1 ? 's' : ''
                    }}
                  </p>
                  <div class="flex flex-wrap gap-2">
                    <Button
                      size="sm"
                      :disabled="
                        savingKey === `tts-save-${tts.id}` || !ttsDraftDirty(tts)
                      "
                      @click="saveTts(tts.id, tts)"
                    >
                      Enregistrer
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      :disabled="tts.profile_count > 0 || savingKey === `tts-${tts.id}`"
                      :title="
                        tts.profile_count > 0
                          ? 'Encore utilisé par des profils'
                          : 'Supprimer ce profil TTS inutilisé'
                      "
                      @click="removeTts(tts)"
                    >
                      Supprimer
                    </Button>
                  </div>
                </div>
                <div class="grid gap-1.5">
                  <Label :for="`tts-nom-${tts.id}`">Nom TTS</Label>
                  <Input
                    :id="`tts-nom-${tts.id}`"
                    :model-value="ensureTtsDraft(tts.id, tts).nom_tts"
                    :disabled="savingKey === `tts-save-${tts.id}`"
                    @update:model-value="
                      ensureTtsDraft(tts.id, tts).nom_tts = String($event)
                    "
                  />
                </div>
                <div class="grid gap-1.5">
                  <Label :for="`tts-desc-${tts.id}`">Description TTS</Label>
                  <textarea
                    :id="`tts-desc-${tts.id}`"
                    :class="textareaClass"
                    :value="ensureTtsDraft(tts.id, tts).description_tts"
                    :disabled="savingKey === `tts-save-${tts.id}`"
                    spellcheck="false"
                    @input="
                      ensureTtsDraft(tts.id, tts).description_tts = (
                        $event.target as HTMLTextAreaElement
                      ).value
                    "
                  />
                </div>
              </div>
              <div class="grid gap-3 border-t border-border pt-4">
                <p class="text-sm font-medium">Nouveau profil TTS</p>
                <div class="grid gap-1.5">
                  <Label for="new-tts">Nom</Label>
                  <Input id="new-tts" v-model="newTtsName" />
                </div>
                <div class="grid gap-1.5">
                  <Label for="new-tts-desc">Description</Label>
                  <textarea
                    id="new-tts-desc"
                    v-model="newTtsDescription"
                    :class="textareaClass"
                    spellcheck="false"
                  />
                </div>
                <Button class="w-fit" :disabled="creating || !newTtsName.trim()" @click="addTts">
                  Créer
                </Button>
              </div>
            </CardContent>
          </Card>

          <Card v-else class="neon-panel h-fit w-full">
            <CardHeader>
              <CardTitle>Loadouts</CardTitle>
            </CardHeader>
            <CardContent class="grid content-start gap-4">
              <p v-if="loadoutCatalog.length === 0" class="text-sm text-muted-foreground">
                Aucun loadout.
              </p>
              <div v-else class="grid gap-4">
                <div
                  v-for="loadout in loadoutCatalog"
                  :key="loadout.id"
                  class="grid gap-2 border-b border-border pb-4 last:border-0 last:pb-0"
                >
                  <div class="flex flex-wrap items-center justify-between gap-2">
                    <div class="min-w-0">
                      <p class="text-sm font-medium break-words">{{ loadout.nom }}</p>
                      <p class="text-xs text-muted-foreground">
                        #{{ loadout.id }} · {{ loadout.profile_count }} profil{{
                          loadout.profile_count > 1 ? 's' : ''
                        }}
                        · {{ loadout.figurine_count }} fig.
                        <template v-if="loadout.silhouette_size">
                          · S{{ loadout.silhouette_size }}
                        </template>
                        <template v-if="loadout.silhouette_type">
                          · {{ loadout.silhouette_type }}
                        </template>
                      </p>
                    </div>
                    <Button
                      variant="outline"
                      size="sm"
                      :disabled="
                        loadout.profile_count > 0 ||
                        loadout.figurine_count > 0 ||
                        savingKey === `loadout-${loadout.id}`
                      "
                      :title="
                        loadout.profile_count > 0 || loadout.figurine_count > 0
                          ? 'Encore utilisé par des profils ou des figurines'
                          : 'Supprimer ce loadout inutilisé'
                      "
                      @click="removeLoadout(loadout)"
                    >
                      Supprimer
                    </Button>
                  </div>
                  <div
                    v-if="figurinesForLoadout(loadout.id).length"
                    class="grid gap-2 rounded-md border border-border/60 p-2"
                  >
                    <p class="text-xs font-medium text-muted-foreground">
                      Figurines (loadout_id)
                    </p>
                    <div
                      v-for="figurine in figurinesForLoadout(loadout.id)"
                      :key="figurine.id"
                      class="grid gap-1.5 sm:grid-cols-[minmax(0,1fr)_minmax(12rem,1fr)] sm:items-center"
                    >
                      <p class="text-sm">
                        {{ figurine.painter || 'Sans peintre' }}
                        <span class="text-xs text-muted-foreground">#{{ figurine.id }}</span>
                      </p>
                      <select
                        :class="selectClass"
                        :disabled="savingKey === `figurine-${figurine.id}`"
                        :value="figurine.loadout_id"
                        :aria-label="`Loadout de la figurine ${figurine.id}`"
                        @change="
                          assignFigurine(
                            figurine,
                            Number(($event.target as HTMLSelectElement).value),
                          )
                        "
                      >
                        <option
                          v-for="option in loadoutOptions"
                          :key="option.id"
                          :value="option.id"
                        >
                          #{{ option.id }} · {{ option.nom }}
                        </option>
                      </select>
                    </div>
                  </div>
                  <p v-else class="text-xs text-muted-foreground">Aucune figurine sur ce loadout.</p>
                </div>
              </div>
              <div class="grid gap-3 border-t border-border pt-4">
                <p class="text-sm font-medium">Nouveau loadout</p>
                <div class="grid gap-1.5">
                  <Label for="new-loadout">Nom</Label>
                  <Input
                    id="new-loadout"
                    v-model="newLoadoutName"
                    placeholder="Combi Rifle, Flash Pulse…"
                  />
                </div>
                <Button
                  class="w-fit"
                  :disabled="creating || !newLoadoutName.trim()"
                  @click="addLoadout"
                >
                  Créer
                </Button>
              </div>
            </CardContent>
          </Card>
        </template>
      </div>
    </template>
  </div>
</template>
