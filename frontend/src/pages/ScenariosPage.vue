<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { Dices } from '@lucide/vue'
import {
  fetchPackCommonRules,
  fetchPackSecondaries,
  fetchPrefs,
  fetchScenarioPack,
  updatePackCommonRule,
  updatePackSecondary,
  updatePrefs,
  updateScenarioPack,
  type SecondaryViewMode,
} from '@/lib/api'
import { pageTitle } from '@/lib/pageTitle'
import { shufflePick } from '@/lib/shufflePick'
import { secondaryImageSrc } from '@/lib/secondaryImages'
import { splitRuleTitle } from '@/lib/ruleTitle'
import {
  DEFAULT_SCENARIO_PACK_SLUG,
  type CommonRule,
  type ScenarioDetail,
  type ScenarioPackPage,
  type SecondaryObjective,
} from '@/types/elo'
import AdminContentEditor from '@/components/AdminContentEditor.vue'
import ImageViewer, {
  type ImageViewerItem,
} from '@/components/ImageViewer.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import ScenarioDetailView from '@/components/ScenarioDetailView.vue'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

type ScenarioTabId = 'liste' | 'regles' | 'secondaires'

const TP_SECTION = '## Calcul des points de tournoi'
const TAB_IDS: ScenarioTabId[] = ['liste', 'regles', 'secondaires']

const route = useRoute()
const router = useRouter()
const { canEditContent } = useAdminEditMode()
const page = ref<ScenarioPackPage | null>(null)
const secondaries = ref<SecondaryObjective[]>([])
const commonRules = ref<CommonRule[]>([])
const loading = ref(true)
const secondariesLoading = ref(false)
const commonRulesLoading = ref(false)
const apiOnline = ref(true)
const activeTab = ref<ScenarioTabId>('liste')
const secondariesLoaded = ref(false)
const commonRulesLoaded = ref(false)
const secondaryViewMode = ref<SecondaryViewMode>('liste')
const drawnSecondarySlugs = ref<string[] | null>(null)
const preferredScenarioSlug = ref<string | null>(null)
const imageViewerOpen = ref(false)
const imageViewerIndex = ref(0)

const tabs = [
  { id: 'liste' as const, label: 'Scénarios' },
  { id: 'secondaires' as const, label: 'Secondaires' },
  { id: 'regles' as const, label: 'Règles' },
]

const activeTabLabel = computed(
  () => tabs.find((tab) => tab.id === activeTab.value)?.label ?? 'Scénarios',
)

const displayedSecondaries = computed(() => {
  const drawn = drawnSecondarySlugs.value
  if (!drawn) return secondaries.value
  const bySlug = new Map(secondaries.value.map((item) => [item.slug, item]))
  return drawn
    .map((slug) => bySlug.get(slug))
    .filter((item): item is SecondaryObjective => item != null)
})

const secondaryViewerItems = computed((): ImageViewerItem[] =>
  displayedSecondaries.value.flatMap((secondary) => {
    const src = secondaryImageSrc(secondary.slug)
    if (!src) return []
    return [
      {
        src,
        alt: secondary.name,
        caption: secondary.name,
      },
    ]
  }),
)

const sortedCommonRules = computed(() =>
  [...commonRules.value].sort((a, b) =>
    a.name.localeCompare(b.name, 'fr', { sensitivity: 'base' }),
  ),
)

const preambleParts = computed(() => {
  const md = page.value?.pack.preamble_md ?? ''
  const idx = md.indexOf(TP_SECTION)
  if (idx < 0) {
    return { before: md, after: '' }
  }
  return {
    before: md.slice(0, idx).trimEnd(),
    after: md.slice(idx).trimStart(),
  }
})

const selectedScenarioSlug = computed(() => {
  const raw = route.query.scenario
  const value = Array.isArray(raw) ? raw[0] : raw
  const scenarios = page.value?.scenarios ?? []
  if (typeof value === 'string' && scenarios.some((s) => s.slug === value)) {
    return value
  }
  if (
    preferredScenarioSlug.value &&
    scenarios.some((s) => s.slug === preferredScenarioSlug.value)
  ) {
    return preferredScenarioSlug.value
  }
  return scenarios[0]?.slug ?? null
})

function tabFromQuery(): ScenarioTabId {
  const raw = route.query.tab
  const value = Array.isArray(raw) ? raw[0] : raw
  if (value && TAB_IDS.includes(value as ScenarioTabId)) {
    return value as ScenarioTabId
  }
  return 'liste'
}

function syncRouteQuery(tab: ScenarioTabId, scenarioSlug?: string | null) {
  const query: Record<string, string> = {}
  if (tab !== 'liste') {
    query.tab = tab
  } else if (scenarioSlug) {
    query.scenario = scenarioSlug
  }
  router.replace({ name: 'scenarios', query })
}

function setActiveTab(tab: ScenarioTabId) {
  activeTab.value = tab
  const scenario =
    tab === 'liste'
      ? (preferredScenarioSlug.value ?? selectedScenarioSlug.value)
      : null
  syncRouteQuery(tab, scenario)
}

function setSelectedScenario(slug: string) {
  preferredScenarioSlug.value = slug
  syncRouteQuery('liste', slug)
  void updatePrefs({ scenario_slug: slug }).catch(() => {
    // Keep the local choice even if persistence fails.
  })
}

function openSecondaryViewer(slug: string) {
  const src = secondaryImageSrc(slug)
  if (!src) return
  const index = secondaryViewerItems.value.findIndex((item) => item.src === src)
  if (index < 0) return
  imageViewerIndex.value = index
  imageViewerOpen.value = true
}

function viewButtonClass(mode: SecondaryViewMode) {
  return secondaryViewMode.value === mode
    ? 'border-primary bg-primary! text-primary-foreground hover:bg-primary/90'
    : 'border-border bg-black text-white hover:text-primary'
}

function setSecondaryViewMode(mode: SecondaryViewMode) {
  const next: SecondaryViewMode =
    secondaryViewMode.value === mode
      ? mode === 'liste'
        ? 'cartes'
        : 'liste'
      : mode
  secondaryViewMode.value = next
  void updatePrefs({ secondary_view_mode: next }).catch(() => {
    // Keep the local choice even if persistence fails.
  })
}

async function loadSecondaries() {
  if (secondariesLoaded.value || secondariesLoading.value) return
  secondariesLoading.value = true
  try {
    secondaries.value = await fetchPackSecondaries(DEFAULT_SCENARIO_PACK_SLUG)
    secondariesLoaded.value = true
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de charger les objectifs secondaires',
    )
  } finally {
    secondariesLoading.value = false
  }
}

async function loadCommonRules() {
  if (commonRulesLoaded.value || commonRulesLoading.value) return
  commonRulesLoading.value = true
  try {
    commonRules.value = await fetchPackCommonRules(DEFAULT_SCENARIO_PACK_SLUG)
    commonRulesLoaded.value = true
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de charger les règles communes',
    )
  } finally {
    commonRulesLoading.value = false
  }
}

async function savePreamble(payload: { body: string }) {
  const pack = await updateScenarioPack(DEFAULT_SCENARIO_PACK_SLUG, {
    preamble_md: payload.body,
  })
  if (page.value) {
    page.value = { ...page.value, pack }
  }
  toast.success('Règles générales enregistrées')
}

async function saveSecondary(
  slug: string,
  payload: { name?: string; body: string },
) {
  const updated = await updatePackSecondary(DEFAULT_SCENARIO_PACK_SLUG, slug, {
    name: payload.name ?? '',
    body_md: payload.body,
  })
  secondaries.value = secondaries.value.map((item) =>
    item.slug === slug ? updated : item,
  )
  toast.success('Objectif secondaire enregistré')
}

async function saveCommonRule(
  slug: string,
  payload: { name?: string; body: string },
) {
  const updated = await updatePackCommonRule(DEFAULT_SCENARIO_PACK_SLUG, slug, {
    name: payload.name ?? '',
    body_md: payload.body,
  })
  commonRules.value = commonRules.value.map((rule) =>
    rule.slug === slug ? updated : rule,
  )
  toast.success('Règle commune enregistrée')
}

function onScenarioLoaded(scenario: ScenarioDetail) {
  document.title = pageTitle(scenario.name)
}

function drawScenario() {
  const scenarios = page.value?.scenarios
  if (!scenarios?.length) return
  const pick = scenarios[Math.floor(Math.random() * scenarios.length)]!
  setSelectedScenario(pick.slug)
}

function drawSecondaries() {
  if (secondaries.value.length < 1) return
  drawnSecondarySlugs.value = shufflePick(secondaries.value, 3).map(
    (item) => item.slug,
  )
}

function clearDrawnSecondaries() {
  drawnSecondarySlugs.value = null
}

onMounted(async () => {
  activeTab.value = tabFromQuery()
  try {
    const [pack, prefs] = await Promise.all([
      fetchScenarioPack(DEFAULT_SCENARIO_PACK_SLUG),
      fetchPrefs().catch(() => null),
    ])
    page.value = pack
    if (prefs?.secondary_view_mode === 'liste' || prefs?.secondary_view_mode === 'cartes') {
      secondaryViewMode.value = prefs.secondary_view_mode
    }
    if (prefs?.scenario_slug) {
      preferredScenarioSlug.value = prefs.scenario_slug
    }
    const queryScenario = Array.isArray(route.query.scenario)
      ? route.query.scenario[0]
      : route.query.scenario
    if (typeof queryScenario === 'string') {
      preferredScenarioSlug.value = queryScenario
    } else if (
      activeTab.value === 'liste' &&
      preferredScenarioSlug.value &&
      pack.scenarios.some((s) => s.slug === preferredScenarioSlug.value)
    ) {
      syncRouteQuery('liste', preferredScenarioSlug.value)
    }
    apiOnline.value = true
  } catch (error) {
    apiOnline.value = false
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de charger le pack de scénarios',
    )
  } finally {
    loading.value = false
  }
  if (activeTab.value === 'secondaires') {
    await loadSecondaries()
  }
  if (activeTab.value === 'regles') {
    await loadCommonRules()
  }
})

watch(
  () => route.query.tab,
  () => {
    activeTab.value = tabFromQuery()
  },
)

watch(activeTab, (tab) => {
  if (tab === 'secondaires') {
    loadSecondaries()
  }
  if (tab === 'regles') {
    loadCommonRules()
  }
  if (tab === 'liste') {
    document.title = pageTitle('Scénarios')
  } else {
    document.title = pageTitle(activeTabLabel.value)
  }
})
</script>

<template>
  <div class="page-stack">
    <nav class="page-title-tabs" aria-label="Sections des scénarios">
      <h1 class="sr-only">{{ activeTabLabel }}</h1>
      <button
        v-for="tab in tabs"
        :key="tab.id"
        type="button"
        class="page-title-tab"
        :class="{ 'page-title-tab--active': activeTab === tab.id }"
        :aria-current="activeTab === tab.id ? 'page' : undefined"
        @click="setActiveTab(tab.id)"
      >
        {{ tab.label }}
      </button>
    </nav>

    <Alert v-if="!apiOnline" variant="destructive" class="neon-panel-accent shrink-0">
      <AlertTitle>API indisponible</AlertTitle>
      <AlertDescription>
        Lancez le serveur Rust puis rechargez la page.
      </AlertDescription>
    </Alert>

    <p v-else-if="loading" class="shrink-0 text-sm text-muted-foreground">
      Chargement…
    </p>

    <template v-else-if="page">
      <div class="tournament-tab-panels page-panel-scroll">
        <template v-if="activeTab === 'liste'">
          <div class="flex flex-wrap items-center justify-between gap-3">
            <nav
              class="tournament-tabs scenario-subtabs"
              aria-label="Scénarios du pack"
            >
              <button
                v-for="scenario in page.scenarios"
                :key="scenario.id"
                type="button"
                class="tournament-tab"
                :class="{
                  'tournament-tab--active':
                    selectedScenarioSlug === scenario.slug,
                }"
                @click="setSelectedScenario(scenario.slug)"
              >
                {{ scenario.name }}
              </button>
            </nav>
            <Button
              variant="outline"
              size="sm"
              class="shrink-0"
              :disabled="page.scenarios.length === 0"
              @click="drawScenario"
            >
              <Dices class="size-4" />
              Tirer au sort
            </Button>
          </div>

          <ScenarioDetailView
            v-if="selectedScenarioSlug"
            :key="selectedScenarioSlug"
            :slug="selectedScenarioSlug"
            @loaded="onScenarioLoaded"
          />
          <p v-else class="text-sm text-muted-foreground">
            Aucun scénario dans ce pack.
          </p>
        </template>

        <template v-else-if="activeTab === 'regles'">
          <div class="grid gap-3 pb-4">
            <Card class="neon-panel shrink-0">
              <CardHeader :class="{ 'pr-24': canEditContent }">
                <CardTitle>Règles générales</CardTitle>
              </CardHeader>
              <CardContent>
                <AdminContentEditor
                  :can-edit="canEditContent"
                  :body="page.pack.preamble_md"
                  :rows="18"
                  :rules="commonRules"
                  :persist="savePreamble"
                >
                  <div class="space-y-4">
                    <MarkdownContent :source="preambleParts.before" />
                    <MarkdownContent
                      v-if="preambleParts.after"
                      :source="preambleParts.after"
                    />
                  </div>
                </AdminContentEditor>
              </CardContent>
            </Card>

            <Card class="neon-panel shrink-0">
              <CardHeader>
                <CardTitle>Règles communes des scénarios</CardTitle>
              </CardHeader>
              <CardContent class="space-y-6">
                <p
                  v-if="commonRulesLoading"
                  class="text-sm text-muted-foreground"
                >
                  Chargement…
                </p>
                <p
                  v-else-if="sortedCommonRules.length === 0"
                  class="text-sm text-muted-foreground"
                >
                  Aucune règle commune.
                </p>
                <section
                  v-for="rule in sortedCommonRules"
                  :id="rule.slug"
                  :key="rule.id"
                  class="relative space-y-2"
                >
                  <h3
                    class="font-semibold"
                    :class="{ 'pr-24': canEditContent }"
                  >
                    <span class="text-primary">{{
                      splitRuleTitle(rule.name).label
                    }}</span>
                    <span
                      v-if="splitRuleTitle(rule.name).suffix"
                      class="text-foreground"
                      >{{ splitRuleTitle(rule.name).suffix }}</span
                    >
                  </h3>
                  <AdminContentEditor
                    :can-edit="canEditContent"
                    :name="rule.name"
                    :body="rule.body_md"
                    :rows="6"
                    :rules="commonRules"
                    :persist="(payload) => saveCommonRule(rule.slug, payload)"
                  >
                    <MarkdownContent
                      :source="rule.body_md"
                      :rules="commonRules"
                    />
                  </AdminContentEditor>
                </section>
              </CardContent>
            </Card>
          </div>
        </template>

        <template v-else>
          <div class="flex flex-wrap items-start justify-between gap-3">
            <p class="page-description shrink-0 max-w-2xl">
              Avant le choix de la liste, chaque joueur pioche 3 secondaires et
              en choisit 1 à réaliser.
            </p>
            <div class="flex flex-wrap items-center gap-3">
              <Button
                variant="outline"
                size="sm"
                class="shrink-0"
                :disabled="secondariesLoading || secondaries.length < 3"
                @click="drawSecondaries"
              >
                <Dices class="size-4" />
                Tirer au sort
              </Button>
              <Button
                v-if="drawnSecondarySlugs"
                variant="ghost"
                size="sm"
                class="shrink-0"
                @click="clearDrawnSecondaries"
              >
                Tous
              </Button>
              <div class="flex items-center gap-2">
                <span class="text-sm font-medium text-muted-foreground"
                  >Affichage :</span
                >
                <div class="flex items-center gap-0">
                  <Button
                    type="button"
                    size="xs"
                    variant="outline"
                    :class="['rounded-r-none', viewButtonClass('liste')]"
                    @click="setSecondaryViewMode('liste')"
                  >
                    Texte
                  </Button>
                  <Button
                    type="button"
                    size="xs"
                    variant="outline"
                    :class="[
                      'rounded-l-none border-l-0',
                      viewButtonClass('cartes'),
                    ]"
                    @click="setSecondaryViewMode('cartes')"
                  >
                    Cartes
                  </Button>
                </div>
              </div>
            </div>
          </div>

          <p
            v-if="secondariesLoading"
            class="text-sm text-muted-foreground"
          >
            Chargement…
          </p>

          <div
            v-else-if="secondaryViewMode === 'cartes'"
            class="secondary-card-grid pb-4"
          >
            <button
              v-for="secondary in displayedSecondaries"
              :id="secondary.slug"
              :key="secondary.id"
              type="button"
              class="neon-panel secondary-card shrink-0 overflow-hidden p-0 text-left disabled:cursor-default disabled:opacity-100"
              :class="{
                'secondary-card--clickable': !!secondaryImageSrc(secondary.slug),
              }"
              :disabled="!secondaryImageSrc(secondary.slug)"
              @click="openSecondaryViewer(secondary.slug)"
            >
              <img
                v-if="secondaryImageSrc(secondary.slug)"
                :src="secondaryImageSrc(secondary.slug)!"
                :alt="secondary.name"
                class="secondary-card-image"
              />
              <p
                v-else
                class="flex aspect-[5/7] items-center justify-center p-4 text-center text-sm font-semibold"
              >
                {{ secondary.name }}
              </p>
            </button>
          </div>

          <div v-else class="grid gap-3 pb-4">
            <Card
              v-for="secondary in displayedSecondaries"
              :id="secondary.slug"
              :key="secondary.id"
              class="neon-panel shrink-0"
            >
              <CardHeader :class="{ 'pr-24': canEditContent }">
                <CardTitle class="text-lg">{{ secondary.name }}</CardTitle>
              </CardHeader>
              <CardContent>
                <AdminContentEditor
                  :can-edit="canEditContent"
                  :name="secondary.name"
                  :body="secondary.body_md"
                  :rows="8"
                  :rules="commonRules"
                  :persist="(payload) => saveSecondary(secondary.slug, payload)"
                >
                  <MarkdownContent :source="secondary.body_md" />
                </AdminContentEditor>
              </CardContent>
            </Card>
          </div>
        </template>
      </div>
    </template>

    <ImageViewer
      v-model:open="imageViewerOpen"
      v-model:index="imageViewerIndex"
      :items="secondaryViewerItems"
    />
  </div>
</template>
