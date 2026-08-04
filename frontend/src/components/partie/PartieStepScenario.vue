<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Dices } from '@lucide/vue'
import type { PartieScenario } from '@/composables/usePartieFlow'
import { fetchPackScenario } from '@/lib/api'
import type { ScenarioDetail, ScenarioSummary } from '@/types/elo'
import { DEFAULT_SCENARIO_PACK_SLUG } from '@/types/elo'
import MarkdownContent from '@/components/MarkdownContent.vue'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

const props = defineProps<{
  scenarios: ScenarioSummary[]
  loading?: boolean
  initial?: PartieScenario | null
}>()

const emit = defineEmits<{
  back: []
  next: [scenario: PartieScenario]
}>()

const selectedSlug = ref<string | undefined>(
  props.initial?.mode === 'other' ? undefined : props.initial?.slug,
)
const otherName = ref(props.initial?.mode === 'other' ? (props.initial.other ?? '') : '')
const scenarioDetail = ref<ScenarioDetail | null>(null)
const detailLoading = ref(false)
const mapFailed = ref(false)

const selectedScenario = computed((): ScenarioSummary | null => {
  if (!selectedSlug.value) return null
  return props.scenarios.find((item) => item.slug === selectedSlug.value) ?? null
})

const canContinue = computed(
  () => Boolean(selectedSlug.value) || otherName.value.trim().length > 0,
)

const showPackPreview = computed(
  () => Boolean(selectedSlug.value && scenarioDetail.value),
)

const mapPreviewSrc = computed(() => {
  const scenario = scenarioDetail.value
  if (!scenario?.map_filename || mapFailed.value) return undefined
  return `/scenario-maps/${scenario.map_filename}`
})

const ruleGlossary = computed(() => {
  const detail = scenarioDetail.value
  if (!detail) return []
  const rules = [...detail.common_rules]
  if (detail.exclusion_rule) {
    rules.push(detail.exclusion_rule)
  }
  return rules
})

watch(selectedSlug, async (value) => {
  if (value) {
    otherName.value = ''
  }
  mapFailed.value = false
  scenarioDetail.value = null
  if (!value) return

  detailLoading.value = true
  try {
    scenarioDetail.value = await fetchPackScenario(DEFAULT_SCENARIO_PACK_SLUG, value)
  } catch {
    scenarioDetail.value = null
  } finally {
    detailLoading.value = false
  }
}, { immediate: true })

watch(otherName, (value) => {
  if (value.trim()) {
    selectedSlug.value = undefined
    scenarioDetail.value = null
  }
})

function drawScenario() {
  if (props.scenarios.length === 0) return
  const pick = props.scenarios[Math.floor(Math.random() * props.scenarios.length)]!
  selectedSlug.value = pick.slug
}

function submit() {
  if (!canContinue.value) return

  const other = otherName.value.trim()
  if (other) {
    emit('next', {
      mode: 'other',
      other,
      name: other,
    })
    return
  }

  const scenario = selectedScenario.value
  if (!scenario) return

  emit('next', {
    mode: 'list',
    id: scenario.id,
    slug: scenario.slug,
    name: scenario.name,
  })
}
</script>

<template>
  <div class="grid gap-6">
    <p class="page-description">
      Choisissez un scénario du pack Poissonnerie ou saisissez un scénario personnalisé.
    </p>

    <div class="grid grid-cols-1 gap-4 lg:grid-cols-2">
      <section class="player-match-panel">
        <p class="player-match-panel-title">La poissonnerie</p>
        <div class="grid gap-3">
          <div class="grid gap-2">
            <Label for="scenario-pack">Scénario du pack</Label>
            <Select v-model="selectedSlug" :disabled="loading">
              <SelectTrigger id="scenario-pack">
                <SelectValue placeholder="Choisir un scénario" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="scenario in scenarios"
                  :key="scenario.slug"
                  :value="scenario.slug"
                >
                  {{ scenario.name }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <Button
            type="button"
            variant="outline"
            class="w-fit"
            :disabled="loading || scenarios.length === 0"
            @click="drawScenario"
          >
            <Dices class="size-4" />
            Tirer au sort
          </Button>
        </div>
      </section>

      <section class="player-match-panel">
        <p class="player-match-panel-title">Autre</p>
        <div class="grid gap-2">
          <Label for="scenario-other">Nom du scénario</Label>
          <Input
            id="scenario-other"
            v-model="otherName"
            placeholder="Ex. Mission maison"
            autocomplete="off"
          />
        </div>
      </section>
    </div>

    <div class="flex flex-col gap-2 sm:flex-row sm:justify-between">
      <Button type="button" variant="outline" @click="emit('back')">
        Précédent
      </Button>
      <Button type="button" :disabled="!canContinue" @click="submit">
        Valider
      </Button>
    </div>

    <div v-if="detailLoading" class="text-sm text-muted-foreground">
      Chargement du scénario…
    </div>

    <div
      v-else-if="showPackPreview && scenarioDetail"
      class="grid grid-cols-1 gap-4 md:grid-cols-2"
    >
      <Card class="neon-panel min-h-0">
        <CardHeader>
          <CardTitle class="text-base">Objectifs</CardTitle>
        </CardHeader>
        <CardContent class="scenario-card-body max-h-80 overflow-y-auto !pb-2 !pr-4 !pl-8">
          <MarkdownContent
            :source="scenarioDetail.objectives_md"
            :rules="ruleGlossary"
          />
        </CardContent>
      </Card>

      <Card class="neon-panel min-h-0 overflow-hidden">
        <CardHeader>
          <CardTitle class="text-base">Carte</CardTitle>
        </CardHeader>
        <CardContent class="flex min-h-0 items-center justify-center pt-0">
          <img
            v-if="mapPreviewSrc"
            :src="mapPreviewSrc"
            :alt="`Carte — ${scenarioDetail.name}`"
            class="scenario-map max-h-80 w-full object-contain"
            @error="mapFailed = true"
          />
          <p v-else class="py-6 text-sm text-muted-foreground">
            Carte indisponible.
          </p>
        </CardContent>
      </Card>
    </div>
  </div>
</template>
