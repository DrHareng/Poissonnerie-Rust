<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import {
  fetchPackCommonRules,
  fetchPackScenario,
  updatePackCommonRule,
  updatePackScenario,
} from '@/lib/api'
import {
  DEFAULT_SCENARIO_PACK_SLUG,
  type CommonRule,
  type ScenarioDetail,
} from '@/types/elo'
import AdminContentEditor from '@/components/AdminContentEditor.vue'
import ImageViewer, {
  type ImageViewerItem,
} from '@/components/ImageViewer.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { withBase } from '@/lib/basePath'
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

const props = defineProps<{
  slug: string
}>()

const emit = defineEmits<{
  loaded: [scenario: ScenarioDetail]
}>()

const { canEditContent } = useAdminEditMode()
const scenario = ref<ScenarioDetail | null>(null)
const packRules = ref<CommonRule[]>([])
const loading = ref(true)
const mapFailed = ref(false)
const imageViewerOpen = ref(false)
const imageViewerIndex = ref(0)
const imageViewerItems = ref<ImageViewerItem[]>([])

const mapSrc = computed(() =>
  scenario.value?.map_filename
    ? withBase(`/scenario-maps/${scenario.value.map_filename}`)
    : null,
)

const ruleGlossary = computed(() => {
  const map = new Map<string, CommonRule>()
  for (const rule of packRules.value) {
    map.set(rule.slug, rule)
  }
  if (scenario.value?.exclusion_rule) {
    map.set(scenario.value.exclusion_rule.slug, scenario.value.exclusion_rule)
  }
  for (const rule of scenario.value?.common_rules ?? []) {
    map.set(rule.slug, rule)
  }
  return [...map.values()]
})

function openImageViewer(item: ImageViewerItem) {
  imageViewerItems.value = [item]
  imageViewerIndex.value = 0
  imageViewerOpen.value = true
}

function openMapViewer() {
  if (!mapSrc.value || !scenario.value) return
  openImageViewer({
    src: mapSrc.value,
    alt: `Carte — ${scenario.value.name}`,
    caption: scenario.value.name,
  })
}

async function load() {
  loading.value = true
  mapFailed.value = false
  scenario.value = null
  try {
    const [detail, rules] = await Promise.all([
      fetchPackScenario(DEFAULT_SCENARIO_PACK_SLUG, props.slug),
      packRules.value.length > 0
        ? Promise.resolve(packRules.value)
        : fetchPackCommonRules(DEFAULT_SCENARIO_PACK_SLUG),
    ])
    scenario.value = detail
    packRules.value = rules
    emit('loaded', scenario.value)
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Scénario introuvable',
    )
  } finally {
    loading.value = false
  }
}

async function saveScenarioField(
  field:
    | 'flavor_text'
    | 'end_condition_md'
    | 'objectives_md'
    | 'deployment_notes_md'
    | 'exclusion_zones_md'
    | 'elements_md'
    | 'special_rules_md',
  body: string,
) {
  scenario.value = await updatePackScenario(
    DEFAULT_SCENARIO_PACK_SLUG,
    props.slug,
    { [field]: body },
  )
  emit('loaded', scenario.value)
  toast.success('Scénario enregistré')
}

async function persistExclusionRule(payload: { name?: string; body: string }) {
  const rule = scenario.value?.exclusion_rule
  if (!rule) return
  await saveCommonRule(rule.slug, payload)
}

async function saveCommonRule(
  ruleSlug: string,
  payload: { name?: string; body: string },
) {
  const updated = await updatePackCommonRule(
    DEFAULT_SCENARIO_PACK_SLUG,
    ruleSlug,
    {
      name: payload.name ?? '',
      body_md: payload.body,
    },
  )
  if (!scenario.value) return
  packRules.value = packRules.value.map((rule) =>
    rule.slug === ruleSlug ? updated : rule,
  )
  if (scenario.value.exclusion_rule?.slug === ruleSlug) {
    scenario.value = {
      ...scenario.value,
      exclusion_rule: updated,
    }
  } else {
    const common_rules = scenario.value.common_rules
      .map((rule) => (rule.slug === ruleSlug ? updated : rule))
      .sort((a, b) =>
        a.name.localeCompare(b.name, 'fr', { sensitivity: 'base' }),
      )
    scenario.value = {
      ...scenario.value,
      common_rules,
    }
  }
  toast.success('Règle commune enregistrée')
}

watch(
  () => props.slug,
  () => {
    void load()
  },
  { immediate: true },
)
</script>

<template>
  <p v-if="loading" class="shrink-0 text-sm text-muted-foreground">
    Chargement…
  </p>

  <template v-else-if="scenario">
    <section class="page-header relative shrink-0">
      <h2 class="page-title text-2xl" :class="{ 'pr-24': canEditContent }">
        {{ scenario.name }}
      </h2>
      <AdminContentEditor
        :can-edit="canEditContent"
        :body="scenario.flavor_text ?? ''"
        :rows="3"
        :markdown="false"
        :persist="(payload) => saveScenarioField('flavor_text', payload.body)"
      >
        <p v-if="scenario.flavor_text" class="page-description italic">
          « {{ scenario.flavor_text }} »
        </p>
      </AdminContentEditor>
    </section>

    <div
      class="scenario-detail-body flex min-h-0 flex-col gap-4 pb-4"
    >
      <div class="scenario-detail-row scenario-detail-row--top">
        <Card class="neon-panel scenario-detail-cell">
          <CardHeader :class="{ 'pr-24': canEditContent }">
            <CardTitle class="text-base">Fin de partie</CardTitle>
          </CardHeader>
          <CardContent class="scenario-card-body !pb-2 !pr-4 !pl-8">
            <AdminContentEditor
              :can-edit="canEditContent"
              :body="scenario.end_condition_md ?? ''"
              :rows="6"
              :rules="ruleGlossary"
              :persist="
                (payload) =>
                  saveScenarioField('end_condition_md', payload.body)
              "
            >
              <MarkdownContent
                :source="scenario.end_condition_md"
                :rules="ruleGlossary"
              />
            </AdminContentEditor>
          </CardContent>
        </Card>

        <Card class="neon-panel scenario-detail-cell">
          <CardHeader :class="{ 'pr-24': canEditContent }">
            <CardTitle class="text-base">Objectifs</CardTitle>
          </CardHeader>
          <CardContent class="scenario-card-body !pb-2 !pr-4 !pl-8">
            <AdminContentEditor
              :can-edit="canEditContent"
              :body="scenario.objectives_md ?? ''"
              :rows="8"
              :rules="ruleGlossary"
              :persist="
                (payload) => saveScenarioField('objectives_md', payload.body)
              "
            >
              <MarkdownContent
                :source="scenario.objectives_md"
                :rules="ruleGlossary"
              />
            </AdminContentEditor>
          </CardContent>
        </Card>
      </div>

      <div class="scenario-detail-row scenario-detail-row--map">
        <Card
          v-if="mapSrc && !mapFailed"
          class="neon-panel scenario-detail-cell overflow-hidden"
        >
          <CardContent class="pt-3">
            <button
              type="button"
              class="scenario-map-button block w-full p-0"
              :aria-label="`Agrandir la carte — ${scenario.name}`"
              @click="openMapViewer"
            >
              <img
                :src="mapSrc"
                :alt="`Carte — ${scenario.name}`"
                class="scenario-map"
                @error="mapFailed = true"
              />
            </button>
          </CardContent>
        </Card>
        <Card
          v-else-if="scenario.map_filename"
          class="neon-panel scenario-detail-cell"
        >
          <CardContent class="py-6 text-sm text-muted-foreground">
            Carte à venir :
            <code class="rounded bg-muted px-1 py-0.5">{{
              scenario.map_filename
            }}</code>
          </CardContent>
        </Card>

        <Card
          v-if="
            scenario.deployment_notes_md ||
            scenario.exclusion_zones_md ||
            canEditContent
          "
          class="neon-panel scenario-detail-cell"
        >
          <CardHeader :class="{ 'pr-24': canEditContent }">
            <CardTitle class="text-base">Déploiement</CardTitle>
          </CardHeader>
          <CardContent class="scenario-card-body space-y-4 !pb-2 !pr-4 !pl-8">
            <section
              v-if="scenario.deployment_notes_md || canEditContent"
              class="relative space-y-2"
            >
              <h3
                class="font-semibold text-primary"
                :class="{ 'pr-24': canEditContent }"
              >
                Éléments à déployer
              </h3>
              <AdminContentEditor
                :can-edit="canEditContent"
                :body="scenario.deployment_notes_md ?? ''"
                :rows="6"
                :rules="ruleGlossary"
                :persist="
                  (payload) =>
                    saveScenarioField('deployment_notes_md', payload.body)
                "
              >
                <MarkdownContent
                  v-if="scenario.deployment_notes_md"
                  :source="scenario.deployment_notes_md"
                  :rules="ruleGlossary"
                />
              </AdminContentEditor>
            </section>

            <section
              v-if="scenario.exclusion_zones_md || canEditContent"
              class="relative space-y-2"
            >
              <h3
                class="font-semibold text-primary"
                :class="{ 'pr-24': canEditContent }"
              >
                {{ scenario.exclusion_rule?.name ?? 'Zones d’exclusion' }}
              </h3>
              <AdminContentEditor
                :can-edit="canEditContent"
                :body="scenario.exclusion_zones_md ?? ''"
                :rows="6"
                :rules="ruleGlossary"
                :persist="
                  (payload) =>
                    saveScenarioField('exclusion_zones_md', payload.body)
                "
              >
                <MarkdownContent
                  v-if="scenario.exclusion_zones_md"
                  :source="scenario.exclusion_zones_md"
                  :rules="ruleGlossary"
                />
              </AdminContentEditor>
              <div v-if="scenario.exclusion_rule" class="relative">
                <AdminContentEditor
                  :can-edit="canEditContent"
                  :name="scenario.exclusion_rule.name"
                  :body="scenario.exclusion_rule.body_md"
                  :rows="4"
                  :rules="ruleGlossary"
                  :persist="persistExclusionRule"
                >
                  <p class="italic text-muted-foreground">
                    {{ scenario.exclusion_rule.body_md }}
                  </p>
                </AdminContentEditor>
              </div>
            </section>
          </CardContent>
        </Card>
      </div>

      <Card
        v-if="scenario.elements_md || canEditContent"
        class="neon-panel shrink-0"
      >
        <CardHeader :class="{ 'pr-24': canEditContent }">
          <CardTitle class="text-base">Éléments de scénario</CardTitle>
        </CardHeader>
        <CardContent class="scenario-card-body !pb-2 !pr-4 !pl-8">
          <AdminContentEditor
            :can-edit="canEditContent"
            :body="scenario.elements_md ?? ''"
            :rows="8"
            :rules="ruleGlossary"
            :persist="
              (payload) => saveScenarioField('elements_md', payload.body)
            "
          >
            <MarkdownContent
              :source="scenario.elements_md"
              :rules="ruleGlossary"
            />
          </AdminContentEditor>
        </CardContent>
      </Card>

      <Card
        v-if="scenario.special_rules_md || canEditContent"
        class="neon-panel shrink-0"
      >
        <CardHeader :class="{ 'pr-24': canEditContent }">
          <CardTitle class="text-base">Règles spéciales</CardTitle>
        </CardHeader>
        <CardContent class="scenario-card-body !pb-2 !pr-4 !pl-8">
          <AdminContentEditor
            :can-edit="canEditContent"
            :body="scenario.special_rules_md ?? ''"
            :rows="8"
            :rules="ruleGlossary"
            :persist="
              (payload) => saveScenarioField('special_rules_md', payload.body)
            "
          >
            <MarkdownContent
              :source="scenario.special_rules_md"
              :rules="ruleGlossary"
            />
          </AdminContentEditor>
        </CardContent>
      </Card>
    </div>

    <ImageViewer
      v-model:open="imageViewerOpen"
      v-model:index="imageViewerIndex"
      :items="imageViewerItems"
    />
  </template>
</template>
