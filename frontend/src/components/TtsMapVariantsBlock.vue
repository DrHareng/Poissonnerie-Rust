<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Download, Plus, Trash2 } from '@lucide/vue'
import { toast } from 'vue-sonner'
import {
  createTtsMapVariant,
  deleteTtsMapVariant,
  fetchScenarioPack,
  fetchTournaments,
  fetchTtsMapVariants,
  fetchTtsMaps,
  uploadTtsMapVariantJson,
} from '@/lib/api'
import { withBase } from '@/lib/basePath'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import type {
  ScenarioSummary,
  TournamentListEntry,
  TtsMapSummary,
  TtsMapVariant,
} from '@/types/elo'
import { DEFAULT_SCENARIO_PACK_SLUG } from '@/types/elo'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

const props = withDefaults(
  defineProps<{
    title?: string
    mapId?: number | null
    scenarioId?: number | null
    tournamentId?: number | null
    scenarioOptions?: ScenarioSummary[]
  }>(),
  {
    title: 'Maps TTS',
  },
)

const { canEditContent } = useAdminEditMode()

const variants = ref<TtsMapVariant[]>([])
const maps = ref<TtsMapSummary[]>([])
const scenarios = ref<ScenarioSummary[]>([])
const tournaments = ref<TournamentListEntry[]>([])
const loading = ref(true)
const creating = ref(false)
const replacingId = ref<number | null>(null)

const selectedMapId = ref('')
const selectedScenarioId = ref('')
const selectedTournamentId = ref('none')
const fileInput = ref<HTMLInputElement | null>(null)
const replaceInput = ref<HTMLInputElement | null>(null)
const pendingFile = ref<File | null>(null)

const scenarioChoices = computed(() =>
  props.scenarioOptions?.length ? props.scenarioOptions : scenarios.value,
)

const showMapSelect = computed(() => props.mapId == null)
const showScenarioSelect = computed(() => props.scenarioId == null)
const showTournamentSelect = computed(() => props.tournamentId == null)

const canSubmit = computed(() => {
  const mapId = props.mapId ?? Number(selectedMapId.value)
  const scenarioId = props.scenarioId ?? Number(selectedScenarioId.value)
  return (
    Number.isFinite(mapId) &&
    mapId > 0 &&
    Number.isFinite(scenarioId) &&
    scenarioId > 0 &&
    pendingFile.value != null &&
    !creating.value
  )
})

function variantLabel(variant: TtsMapVariant) {
  const parts: string[] = []
  if (showScenarioSelect.value) parts.push(variant.scenario_name)
  if (showMapSelect.value) parts.push(variant.map_name)
  if (variant.tournament_name) parts.push(variant.tournament_name)
  else if (showTournamentSelect.value) parts.push('Hors tournoi')
  return parts.join(' · ')
}

async function loadVariants() {
  loading.value = true
  try {
    variants.value = await fetchTtsMapVariants({
      mapId: props.mapId ?? undefined,
      scenarioId: props.scenarioId ?? undefined,
      tournamentId: props.tournamentId ?? undefined,
    })
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de charger les dérivées TTS',
    )
  } finally {
    loading.value = false
  }
}

async function loadOptions() {
  if (!canEditContent.value) return
  const jobs: Promise<void>[] = []
  if (showMapSelect.value && maps.value.length === 0) {
    jobs.push(
      fetchTtsMaps()
        .then((items) => {
          maps.value = items
        })
        .catch(() => {
          maps.value = []
        }),
    )
  }
  if (showScenarioSelect.value && !props.scenarioOptions?.length && scenarios.value.length === 0) {
    jobs.push(
      fetchScenarioPack(DEFAULT_SCENARIO_PACK_SLUG)
        .then((pack) => {
          scenarios.value = pack.scenarios
        })
        .catch(() => {
          scenarios.value = []
        }),
    )
  }
  if (showTournamentSelect.value && tournaments.value.length === 0) {
    jobs.push(
      fetchTournaments()
        .then((items) => {
          tournaments.value = items
        })
        .catch(() => {
          tournaments.value = []
        }),
    )
  }
  await Promise.all(jobs)
}

function onFileChange(event: Event) {
  const input = event.target as HTMLInputElement
  pendingFile.value = input.files?.[0] ?? null
}

async function createVariant() {
  const mapId = props.mapId ?? Number(selectedMapId.value)
  const scenarioId = props.scenarioId ?? Number(selectedScenarioId.value)
  const tournamentId =
    props.tournamentId ??
    (selectedTournamentId.value === 'none'
      ? null
      : Number(selectedTournamentId.value))
  const file = pendingFile.value
  if (!file || !Number.isFinite(mapId) || mapId <= 0 || !Number.isFinite(scenarioId) || scenarioId <= 0) {
    toast.error('Choisissez une map, un scénario et un JSON')
    return
  }
  creating.value = true
  try {
    const created = await createTtsMapVariant({
      mapId,
      scenarioId,
      tournamentId:
        tournamentId != null && Number.isFinite(tournamentId) && tournamentId > 0
          ? tournamentId
          : null,
      file,
    })
    variants.value = [...variants.value, created]
    pendingFile.value = null
    if (fileInput.value) fileInput.value.value = ''
    toast.success('Dérivée TTS ajoutée')
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible d’ajouter la dérivée',
    )
  } finally {
    creating.value = false
  }
}

function askReplace(variant: TtsMapVariant) {
  replacingId.value = variant.id
  replaceInput.value?.click()
}

async function onReplaceSelected(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  const id = replacingId.value
  input.value = ''
  replacingId.value = null
  if (!file || id == null) return
  try {
    const updated = await uploadTtsMapVariantJson(id, file)
    variants.value = variants.value.map((item) =>
      item.id === id ? updated : item,
    )
    toast.success('JSON mis à jour')
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible d’envoyer le JSON',
    )
  }
}

async function removeVariant(variant: TtsMapVariant) {
  if (!window.confirm(`Supprimer la dérivée « ${variantLabel(variant)} » ?`)) {
    return
  }
  try {
    await deleteTtsMapVariant(variant.id)
    variants.value = variants.value.filter((item) => item.id !== variant.id)
    toast.success('Dérivée TTS supprimée')
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de supprimer la dérivée',
    )
  }
}

onMounted(async () => {
  await loadVariants()
  await loadOptions()
})

watch(
  () => [props.mapId, props.scenarioId, props.tournamentId],
  () => {
    void loadVariants()
  },
)

watch(canEditContent, (canEdit) => {
  if (canEdit) void loadOptions()
})
</script>

<template>
  <Card
    v-if="variants.length > 0 || canEditContent"
    class="neon-panel shrink-0 overflow-visible"
  >
    <CardHeader>
      <CardTitle>{{ title }}</CardTitle>
      <CardDescription>
        JSON TTS lié à un scénario, une map, et éventuellement un tournoi.
      </CardDescription>
    </CardHeader>
    <CardContent class="space-y-4">
      <p v-if="loading" class="text-sm text-muted-foreground">Chargement…</p>
      <p
        v-else-if="variants.length === 0"
        class="text-sm text-muted-foreground"
      >
        Aucune dérivée TTS pour le moment.
      </p>
      <ul v-else class="grid gap-2">
        <li
          v-for="variant in variants"
          :key="variant.id"
          class="flex flex-wrap items-center gap-2 rounded-md border border-border/70 px-3 py-2"
        >
          <div class="min-w-0 flex-1 space-y-0.5">
            <p class="truncate text-sm font-medium">
              <RouterLink
                v-if="showScenarioSelect && variant.scenario_slug"
                :to="{ name: 'scenarios', query: { scenario: variant.scenario_slug } }"
                class="text-primary hover:underline"
              >
                {{ variant.scenario_name }}
              </RouterLink>
              <template v-else-if="showScenarioSelect">
                {{ variant.scenario_name }}
              </template>
              <template v-if="showScenarioSelect && showMapSelect"> · </template>
              <RouterLink
                v-if="showMapSelect"
                :to="{ name: 'ressources', query: { map: variant.map_slug } }"
                class="text-primary hover:underline"
              >
                {{ variant.map_name }}
              </RouterLink>
            </p>
            <p class="truncate text-xs text-muted-foreground">
              <RouterLink
                v-if="showTournamentSelect && variant.tournament_id"
                :to="{ name: 'tournoi', params: { id: variant.tournament_id } }"
                class="text-primary hover:underline"
              >
                {{ variant.tournament_name }}
              </RouterLink>
              <span v-else-if="showTournamentSelect">Hors tournoi</span>
              <span v-if="variant.json_filename">
                <template v-if="showTournamentSelect"> · </template>
                {{ variant.json_filename }}
              </span>
            </p>
          </div>
          <Button
            as="a"
            :href="withBase(variant.json_url)"
            size="sm"
            variant="outline"
          >
            <Download class="size-4" />
            JSON
          </Button>
          <template v-if="canEditContent">
            <Button
              type="button"
              size="sm"
              variant="ghost"
              @click="askReplace(variant)"
            >
              Remplacer
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              @click="removeVariant(variant)"
            >
              <Trash2 class="size-3.5" />
              Supprimer
            </Button>
          </template>
        </li>
      </ul>

      <form
        v-if="canEditContent"
        class="grid gap-3 border-t border-border/60 pt-3"
        @submit.prevent="createVariant"
      >
        <div
          class="grid gap-3"
          :class="{
            'sm:grid-cols-2': showMapSelect || showScenarioSelect || showTournamentSelect,
            'lg:grid-cols-3':
              Number(showMapSelect) +
                Number(showScenarioSelect) +
                Number(showTournamentSelect) >
              2,
          }"
        >
          <div v-if="showMapSelect" class="grid gap-1">
            <Label>Map</Label>
            <Select v-model="selectedMapId">
              <SelectTrigger>
                <SelectValue placeholder="Choisir une map" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="map in maps"
                  :key="map.id"
                  :value="String(map.id)"
                >
                  {{ map.name }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div v-if="showScenarioSelect" class="grid gap-1">
            <Label>Scénario</Label>
            <Select v-model="selectedScenarioId">
              <SelectTrigger>
                <SelectValue placeholder="Choisir un scénario" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="scenario in scenarioChoices"
                  :key="scenario.id"
                  :value="String(scenario.id)"
                >
                  {{ scenario.name }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div v-if="showTournamentSelect" class="grid gap-1">
            <Label>Tournoi (optionnel)</Label>
            <Select v-model="selectedTournamentId">
              <SelectTrigger>
                <SelectValue placeholder="Aucun tournoi" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="none">Aucun tournoi</SelectItem>
                <SelectItem
                  v-for="tournament in tournaments"
                  :key="tournament.id"
                  :value="String(tournament.id)"
                >
                  {{ tournament.name }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>
        <div class="flex flex-wrap items-end gap-2">
          <div class="grid min-w-0 flex-1 gap-1">
            <Label for="tts-variant-json">JSON</Label>
            <input
              id="tts-variant-json"
              ref="fileInput"
              type="file"
              accept=".json,application/json"
              class="text-sm text-muted-foreground file:mr-3 file:rounded-md file:border-0 file:bg-primary/15 file:px-3 file:py-1.5 file:text-sm file:font-medium file:text-primary"
              @change="onFileChange"
            />
          </div>
          <Button type="submit" size="sm" :disabled="!canSubmit">
            <Plus class="size-4" />
            {{ creating ? 'Ajout…' : 'Ajouter' }}
          </Button>
        </div>
      </form>
      <input
        ref="replaceInput"
        type="file"
        accept=".json,application/json"
        class="sr-only"
        @change="onReplaceSelected"
      />
    </CardContent>
  </Card>
</template>
