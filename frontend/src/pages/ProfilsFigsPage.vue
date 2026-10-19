<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
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
const search = ref('')
const painterFilter = ref('')
const painterOptions = ref<{ value: string; label: string }[]>([])
const figurineIscs = ref<string[]>([])
const figFilter = ref<FigFilter>('tous')
const figsPanel = ref<{ openCreate: () => void } | null>(null)

const isProfilesTab = computed(() => route.name === 'profils-figs-profils')
const isFigsTab = computed(() => route.name === 'profils-figs-figs')
const isListsTab = computed(() => route.name === 'profils-figs-listes')

const showSearch = computed(() => {
  if (isProfilesTab.value) return !!armyId.value
  if (!isFigsTab.value) return false
  return !!armyId.value || !!painterFilter.value.trim()
})

const showSectorialBar = computed(() => !isListsTab.value)

const pickerArmies = computed(() => {
  const known = new Set(armies.value.map((army) => army.id))
  return [...armies.value, ...extraArmies.value.filter((army) => !known.has(army.id))]
    .filter((army) => isListableArmy(army))
})

const filterQuery = computed(() => {
  const query: Record<string, string> = {}
  const sectorielle = armyId.value?.trim()
  if (sectorielle) query[QUERY_SECTORIELLE] = sectorielle
  const peintre = painterFilter.value.trim()
  if (peintre) query[QUERY_PEINTRE] = peintre
  const recherche = search.value.trim()
  if (recherche) query[QUERY_RECHERCHE] = recherche
  if (figFilter.value !== 'tous') query[QUERY_FIGS] = figFilter.value
  return query
})

const tabs = computed((): PageTitleTab[] =>
  profilsFigsTabs.map((tab) => ({
    ...tab,
    to: {
      ...(typeof tab.to === 'object' ? tab.to : { path: String(tab.to) }),
      query: filterQuery.value,
    },
  })),
)

let writingQuery = false
let applyingFromRoute = false
let filtersReady = false
let searchSyncTimer: ReturnType<typeof setTimeout> | null = null

function firstQueryValue(raw: unknown): string {
  const value = Array.isArray(raw) ? raw[0] : raw
  return typeof value === 'string' ? value.trim() : ''
}

function parseFigFilter(raw: string): FigFilter {
  if (raw === 'avec' || raw === 'sans') return raw
  return 'tous'
}

function readFiltersFromRoute(options?: { clearMissingSectorielle?: boolean }) {
  const sectorielle = firstQueryValue(route.query[QUERY_SECTORIELLE])
  if (sectorielle) setArmyId(sectorielle)
  else if (options?.clearMissingSectorielle) setArmyId(undefined)
  painterFilter.value = firstQueryValue(route.query[QUERY_PEINTRE])
  search.value = firstQueryValue(route.query[QUERY_RECHERCHE])
  figFilter.value = parseFigFilter(firstQueryValue(route.query[QUERY_FIGS]))
}

function sameFilterQuery(
  left: Record<string, string>,
  right: Record<string, unknown>,
) {
  const keys = new Set([
    ...Object.keys(left),
    QUERY_SECTORIELLE,
    QUERY_PEINTRE,
    QUERY_RECHERCHE,
    QUERY_FIGS,
  ])
  for (const key of keys) {
    const a = left[key] ?? ''
    const b = firstQueryValue(right[key])
    if (a !== b) return false
  }
  return true
}

function persistFilters(mode: 'push' | 'replace') {
  if (!filtersReady || applyingFromRoute) return
  if (typeof route.name !== 'string' || !route.name.startsWith('profils-figs')) return
  const query = filterQuery.value
  if (sameFilterQuery(query, route.query)) return
  writingQuery = true
  const navigate = mode === 'push' ? router.push : router.replace
  void navigate({ name: route.name, query }).finally(() => {
    writingQuery = false
  })
}

function scheduleSearchPersist() {
  if (!filtersReady || applyingFromRoute) return
  if (searchSyncTimer != null) clearTimeout(searchSyncTimer)
  searchSyncTimer = setTimeout(() => {
    searchSyncTimer = null
    persistFilters('push')
  }, 300)
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
  () => route.query,
  () => {
    if (writingQuery || !filtersReady) return
    if (typeof route.name !== 'string' || !route.name.startsWith('profils-figs')) return
    applyingFromRoute = true
    try {
      readFiltersFromRoute({ clearMissingSectorielle: true })
    } finally {
      applyingFromRoute = false
    }
  },
)

watch(armyId, () => persistFilters('push'))
watch(painterFilter, () => persistFilters('push'))
watch(figFilter, () => persistFilters('push'))
watch(search, () => scheduleSearchPersist())

onMounted(async () => {
  readFiltersFromRoute({ clearMissingSectorielle: false })
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
    filtersReady = true
    persistFilters('replace')
  } catch (error) {
    filtersReady = true
    pickerError.value = error instanceof Error ? error.message : 'Impossible de charger les sectorielles'
  }
})

onBeforeUnmount(() => {
  if (searchSyncTimer != null) clearTimeout(searchSyncTimer)
})
</script>

<template>
  <div class="page-stack">
    <PageTitleTabs
      ariaLabel="Profils et figurines"
      :tabs="tabs"
    />

    <div v-if="showSectorialBar" class="space-y-1.5">
      <div class="flex flex-wrap items-end gap-2">
        <div class="grid min-w-56 flex-1 gap-1.5 sm:max-w-md">
          <Label>Sectorielle</Label>
          <SectorialPicker
            :model-value="armyId"
            :armies="pickerArmies"
            :clearable="isFigsTab"
            placeholder="Choisir une sectorielle"
            @update:model-value="setArmyId"
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
          v-model="search"
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
      v-model:search="search"
      :army-id="armyId"
      :fig-filter="figFilter"
      :figurine-iscs="figurineIscs"
    />

    <TtsFigurinesPanel
      v-else-if="isFigsTab"
      ref="figsPanel"
      v-model:search="search"
      v-model:painter="painterFilter"
      :army-id="armyId"
      @saved="refreshPainterOptions"
    />

    <TtsListsPanel
      v-else-if="isListsTab"
      :painter-options="painterOptions"
      @update:army-id="setArmyId"
    />
  </div>
</template>
