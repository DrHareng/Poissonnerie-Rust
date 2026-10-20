<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, watch } from 'vue'
import { useRoute, useRouter, type LocationQuery, type LocationQueryRaw } from 'vue-router'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import PlayerPicker from '@/components/PlayerPicker.vue'
import SectorialPicker from '@/components/SectorialPicker.vue'
import TtsFigurinesPanel from '@/components/TtsFigurinesPanel.vue'
import TtsListsPanel from '@/components/TtsListsPanel.vue'
import TtsProfilesPanel from '@/components/TtsProfilesPanel.vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { useArmies } from '@/composables/useArmies'
import { useTtsSectorial } from '@/composables/useTtsSectorial'
import { fetchTtsFigurines, fetchTtsProfileArmies } from '@/lib/api'
import { isListableArmy } from '@/lib/army'
import { profilsFigsTabs, type PageTitleTab } from '@/lib/pageTitleTabs'
import type { Army } from '@/types/elo'

type FigFilter = 'tous' | 'avec' | 'sans'

const QUERY_SECTORIELLE = 'sectorielle'
const QUERY_PEINTRE = 'peintre'
const QUERY_RECHERCHE = 'recherche'
const QUERY_FIGS = 'figs'

const route = useRoute()
const router = useRouter()
const { armies, ensureLoaded } = useArmies()
const { canEditContent } = useAdminEditMode()
const { armyId, ensureLoaded: ensureSectorial, setArmyId } = useTtsSectorial()
const extraArmies = ref<Army[]>([])
const pickerError = ref('')
const painterOptions = ref<{ value: string; label: string }[]>([])
const figurineIscs = ref<string[]>([])
const figsPanel = ref<{ openCreate: () => void } | null>(null)

/** Champ contrôlé localement ; l’URL est mise à jour (replace) à chaque changement. */
const searchDraft = ref('')
const filtersReady = ref(false)
let searchSyncTimer: ReturnType<typeof setTimeout> | null = null
let writingQuery = false

const isProfilesTab = computed(() => route.name === 'profils-figs-profils')
const isFigsTab = computed(() => route.name === 'profils-figs-figs')
const isListsTab = computed(() => route.name === 'profils-figs-listes')

const showSearch = computed(() => {
  if (isFigsTab.value) return true
  if (isProfilesTab.value) return !!armyId.value
  return false
})

const showSectorialBar = computed(() => !isListsTab.value)

const pickerArmies = computed(() => {
  const known = new Set(armies.value.map((army) => army.id))
  return [...armies.value, ...extraArmies.value.filter((army) => !known.has(army.id))]
    .filter((army) => isListableArmy(army))
})

function firstQueryValue(raw: unknown): string {
  const value = Array.isArray(raw) ? raw[0] : raw
  return typeof value === 'string' ? value.trim() : ''
}

function queryFromRoute(query: LocationQuery = route.query) {
  return {
    sectorielle: firstQueryValue(query[QUERY_SECTORIELLE]),
    peintre: firstQueryValue(query[QUERY_PEINTRE]),
    recherche: firstQueryValue(query[QUERY_RECHERCHE]),
    figs: parseFigFilter(firstQueryValue(query[QUERY_FIGS])),
  }
}

function parseFigFilter(raw: string): FigFilter {
  if (raw === 'avec' || raw === 'sans') return raw
  return 'tous'
}

function buildFilterQuery(overrides: {
  sectorielle?: string
  peintre?: string
  recherche?: string
  figs?: FigFilter
} = {}): LocationQueryRaw {
  const current = queryFromRoute()
  const sectorielle = (overrides.sectorielle ?? armyId.value ?? current.sectorielle).trim()
  const peintre = (overrides.peintre ?? painterFilter.value ?? current.peintre).trim()
  const recherche = (overrides.recherche ?? searchDraft.value ?? current.recherche).trim()
  const figs = overrides.figs ?? figFilter.value ?? current.figs

  const query: LocationQueryRaw = {}
  if (sectorielle) query[QUERY_SECTORIELLE] = sectorielle
  if (peintre) query[QUERY_PEINTRE] = peintre
  if (recherche) query[QUERY_RECHERCHE] = recherche
  if (figs !== 'tous') query[QUERY_FIGS] = figs
  return query
}

const painterFilter = computed({
  get: () => queryFromRoute().peintre,
  set: (value: string) => {
    void writeFilters({ peintre: value.trim() }, 'push')
  },
})

const figFilter = computed({
  get: () => queryFromRoute().figs,
  set: (value: FigFilter) => {
    void writeFilters({ figs: value }, 'push')
  },
})

const filterQuery = computed(() => {
  const query = buildFilterQuery()
  const out: Record<string, string> = {}
  for (const [key, value] of Object.entries(query)) {
    if (typeof value === 'string' && value.trim()) out[key] = value.trim()
  }
  return out
})

const tabs = computed((): PageTitleTab[] =>
  profilsFigsTabs.map((tab) => {
    const base = typeof tab.to === 'object' ? tab.to : { path: String(tab.to) }
    return {
      ...tab,
      to: {
        ...base,
        query: { ...filterQuery.value },
      },
    }
  }),
)

function sameFilterQuery(left: LocationQueryRaw, right: LocationQuery) {
  const keys = [QUERY_SECTORIELLE, QUERY_PEINTRE, QUERY_RECHERCHE, QUERY_FIGS]
  for (const key of keys) {
    const a = firstQueryValue(left[key])
    const b = firstQueryValue(right[key])
    if (a !== b) return false
  }
  return true
}

async function writeFilters(
  overrides: Parameters<typeof buildFilterQuery>[0],
  mode: 'push' | 'replace',
  name = typeof route.name === 'string' ? route.name : undefined,
) {
  if (!filtersReady.value) return
  if (!name?.startsWith('profils-figs')) return
  const query = buildFilterQuery(overrides)
  if (name === route.name && sameFilterQuery(query, route.query)) return
  writingQuery = true
  try {
    const navigate = mode === 'push' ? router.push : router.replace
    await navigate({ name, query })
  } finally {
    writingQuery = false
  }
}

function syncSearchDraftFromRoute() {
  searchDraft.value = queryFromRoute().recherche
}

function scheduleSearchUrlSync() {
  if (!filtersReady.value) return
  if (searchSyncTimer != null) clearTimeout(searchSyncTimer)
  searchSyncTimer = setTimeout(() => {
    searchSyncTimer = null
    void writeFilters({ recherche: searchDraft.value }, 'replace')
  }, 200)
}

function flushSearchToUrl() {
  // Annule le debounce : les liens d’onglets lisent déjà searchDraft via filterQuery.
  if (searchSyncTimer != null) {
    clearTimeout(searchSyncTimer)
    searchSyncTimer = null
  }
  const expected = searchDraft.value.trim()
  if (expected === queryFromRoute().recherche) return
  void writeFilters({ recherche: searchDraft.value }, 'replace')
}

function onSectorialChange(value: string | undefined) {
  setArmyId(value)
  void writeFilters({ sectorielle: value?.trim() || '' }, 'push')
}

async function refreshPainterOptions() {
  try {
    const rows = await fetchTtsFigurines()
    const seenPainters = new Set<string>()
    const seenIscs = new Set<string>()
    const options: { value: string; label: string }[] = []
    for (const row of rows) {
      const isc = row.isc.trim().toLowerCase()
      if (isc) seenIscs.add(isc)
      const painter = row.painter.trim()
      if (!painter) continue
      const key = painter.toLowerCase()
      if (seenPainters.has(key)) continue
      seenPainters.add(key)
      options.push({ value: painter, label: painter })
    }
    options.sort((left, right) => left.label.localeCompare(right.label, 'fr', { sensitivity: 'base' }))
    painterOptions.value = options
    figurineIscs.value = [...seenIscs]
  } catch {
    painterOptions.value = []
    figurineIscs.value = []
  }
}

watch(
  () => [route.name, route.fullPath] as const,
  () => {
    if (writingQuery) return
    if (typeof route.name !== 'string' || !route.name.startsWith('profils-figs')) return
    const { sectorielle } = queryFromRoute()
    if (sectorielle) setArmyId(sectorielle)
    syncSearchDraftFromRoute()
  },
)

watch(searchDraft, () => {
  if (!filtersReady.value || writingQuery) return
  if (searchDraft.value.trim() === queryFromRoute().recherche) return
  scheduleSearchUrlSync()
})

onMounted(async () => {
  const fromRoute = queryFromRoute()
  if (fromRoute.sectorielle) setArmyId(fromRoute.sectorielle)
  syncSearchDraftFromRoute()
  try {
    await Promise.all([
      ensureLoaded(),
      ensureSectorial(),
      refreshPainterOptions(),
      fetchTtsProfileArmies().then((counts) => {
        extraArmies.value = counts.flatMap((entry) => {
          if (entry.army_id == null || !entry.name || !entry.slug) return []
          return [{
            id: entry.army_id,
            parent_id: entry.army_id,
            name: entry.name,
            slug: entry.slug,
            logo_url: '',
            discontinued: false,
          }]
        })
      }),
    ])
    filtersReady.value = true
    await writeFilters({}, 'replace')
  } catch (error) {
    filtersReady.value = true
    pickerError.value = error instanceof Error ? error.message : 'Impossible de charger les sectorielles'
  }
})

onBeforeUnmount(() => {
  if (searchSyncTimer != null) clearTimeout(searchSyncTimer)
})
</script>

<template>
  <div
    class="page-stack"
    :class="isFigsTab ? 'min-h-0 flex-1 lg:overflow-hidden' : undefined"
  >
    <div :class="isFigsTab ? 'shrink-0' : undefined" @click.capture="flushSearchToUrl">
      <PageTitleTabs
        ariaLabel="Profils et figurines"
        :tabs="tabs"
      />
    </div>

    <div
      v-if="showSectorialBar"
      class="space-y-1.5"
      :class="isFigsTab ? 'shrink-0' : undefined"
    >
      <div class="flex flex-wrap items-end gap-2">
        <div class="grid min-w-56 flex-1 gap-1.5 sm:max-w-md">
          <Label>Sectorielle</Label>
          <SectorialPicker
            :model-value="armyId"
            :armies="pickerArmies"
            :clearable="isFigsTab"
            placeholder="Choisir une sectorielle"
            @update:model-value="onSectorialChange"
          />
        </div>
        <div v-if="isFigsTab" class="grid min-w-44 flex-1 gap-1.5 sm:max-w-xs">
          <Label>Peintre</Label>
          <PlayerPicker
            :model-value="painterFilter || undefined"
            :options="painterOptions"
            allow-custom
            placeholder="Filtrer par peintre"
            empty-message="Aucun peintre connu."
            @update:model-value="painterFilter = $event ?? ''"
          />
        </div>
        <div v-if="isProfilesTab && armyId" class="grid min-w-36 gap-1.5">
          <Label>Figurines</Label>
          <Select :model-value="figFilter" @update:model-value="figFilter = ($event as FigFilter) ?? 'tous'">
            <SelectTrigger class="w-full min-w-36">
              <SelectValue placeholder="Figurines" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="tous">Tous</SelectItem>
              <SelectItem value="avec">Avec fig</SelectItem>
              <SelectItem value="sans">Sans fig</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <Input
          v-if="showSearch"
          v-model="searchDraft"
          class="w-full max-w-xs"
          :placeholder="isFigsTab ? 'ISC, loadout, peintre…' : 'ISC, loadout, nom…'"
          autocomplete="off"
        />
        <Button
          v-if="isFigsTab && canEditContent"
          type="button"
          @click="figsPanel?.openCreate()"
        >
          Ajouter
        </Button>
      </div>
      <p v-if="pickerError" class="text-sm text-destructive">{{ pickerError }}</p>
    </div>

    <TtsProfilesPanel
      v-if="isProfilesTab"
      v-model:search="searchDraft"
      :army-id="armyId"
      :fig-filter="figFilter"
      :figurine-iscs="figurineIscs"
      :filter-query="filterQuery"
    />

    <div v-else-if="isFigsTab" class="flex min-h-0 flex-1 flex-col overflow-hidden">
      <TtsFigurinesPanel
        ref="figsPanel"
        v-model:search="searchDraft"
        v-model:painter="painterFilter"
        :army-id="armyId"
        @saved="refreshPainterOptions"
      />
    </div>

    <TtsListsPanel
      v-else-if="isListsTab"
      :painter-options="painterOptions"
      @update:army-id="onSectorialChange"
    />
  </div>
</template>
