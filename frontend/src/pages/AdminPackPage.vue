<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { FileText, X } from '@lucide/vue'
import { toast } from 'vue-sonner'
import { fetchScenarioPack } from '@/lib/api'
import { adminTabs } from '@/lib/pageTitleTabs'
import { downloadMissionPack } from '@/lib/tournamentPackDownload'
import { useAuth } from '@/composables/useAuth'
import {
  DEFAULT_SCENARIO_PACK_SLUG,
  type ScenarioSummary,
} from '@/types/elo'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
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

type ScenarioRow = {
  key: number
  slug: string
}

const { isAdmin, loading: authLoading, initialized } = useAuth()

const scenarios = ref<ScenarioSummary[]>([])
const loading = ref(true)
const exporting = ref<'docx' | 'pdf' | null>(null)
const tournamentName = ref('')
let nextRowKey = 1
const rows = ref<ScenarioRow[]>([{ key: nextRowKey++, slug: '' }])

const selectedSlugs = computed(() =>
  rows.value.map((row) => row.slug).filter((slug) => slug.length > 0),
)

const canDownload = computed(
  () =>
    !loading.value
    && !exporting.value
    && tournamentName.value.trim().length > 0
    && selectedSlugs.value.length > 0,
)

function optionsFor(key: number) {
  const current = rows.value.find((row) => row.key === key)?.slug
  const used = new Set(
    rows.value.flatMap((row) =>
      row.key !== key && row.slug ? [row.slug] : [],
    ),
  )
  return scenarios.value.filter(
    (scenario) => scenario.slug === current || !used.has(scenario.slug),
  )
}

function onSelect(key: number, value: unknown) {
  const slug = typeof value === 'string' ? value : ''
  const next = rows.value.map((row) =>
    row.key === key ? { ...row, slug } : row,
  )
  const last = next[next.length - 1]
  if (last?.slug) next.push({ key: nextRowKey++, slug: '' })
  rows.value = next
}

function removeRow(key: number) {
  const next = rows.value.filter((row) => row.key !== key)
  if (next.length === 0 || next[next.length - 1]?.slug) {
    next.push({ key: nextRowKey++, slug: '' })
  }
  rows.value = next
}

async function load() {
  if (!isAdmin.value) {
    loading.value = false
    return
  }
  loading.value = true
  try {
    const page = await fetchScenarioPack(DEFAULT_SCENARIO_PACK_SLUG)
    scenarios.value = page.scenarios
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de charger les scénarios',
    )
  } finally {
    loading.value = false
  }
}

async function download(format: 'docx' | 'pdf') {
  const name = tournamentName.value.trim()
  const slugs = [...selectedSlugs.value]
  if (!name || slugs.length === 0 || exporting.value) {
    return
  }
  exporting.value = format
  try {
    const missing = await downloadMissionPack(format, name, slugs)
    if (missing > 0) {
      toast.message(
        missing === 1
          ? 'Une image n’a pas pu être incluse.'
          : 'Certaines images n’ont pas pu être incluses.',
      )
    }
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de générer le document',
    )
  } finally {
    exporting.value = null
  }
}

watch(
  [initialized, isAdmin],
  ([ready, admin]) => {
    if (ready && admin) void load()
  },
  { immediate: true },
)
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

    <Card v-else class="neon-panel max-w-2xl">
      <CardHeader>
        <CardTitle class="flex items-center gap-2">
          <FileText class="size-5 text-primary" />
          Pack de missions
        </CardTitle>
        <CardDescription>
          Le document enchaîne la page de garde, la table des matières, les
          règles générales avec le récapitulatif des objectifs secondaires, les
          scénarios, puis les règles communes qu’ils utilisent. Les mots-clés
          renvoient vers ces règles.
        </CardDescription>
      </CardHeader>
      <CardContent class="grid gap-4">
        <p v-if="loading" class="text-sm text-muted-foreground">Chargement…</p>
        <template v-else>
          <div class="grid gap-2">
            <Label for="tournament-name">Nom du tournoi</Label>
            <Input
              id="tournament-name"
              v-model="tournamentName"
              placeholder="Nom du tournoi"
              autocomplete="off"
            />
          </div>

          <div class="grid gap-2">
            <Label>Scénarios</Label>
            <div
              v-for="row in rows"
              :key="row.key"
              class="flex items-center gap-2"
            >
              <div class="min-w-0 flex-1">
                <Select
                  :model-value="row.slug || undefined"
                  @update:model-value="(value) => onSelect(row.key, value)"
                >
                  <SelectTrigger class="w-full">
                    <SelectValue placeholder="Choisir un scénario" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem
                      v-for="scenario in optionsFor(row.key)"
                      :key="scenario.id"
                      :value="scenario.slug"
                    >
                      {{ scenario.name }}
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <Button
                v-if="row.slug"
                type="button"
                variant="ghost"
                size="icon"
                class="shrink-0 text-destructive hover:bg-destructive/10 hover:text-destructive"
                aria-label="Retirer ce scénario"
                @click="removeRow(row.key)"
              >
                <X class="size-4" />
              </Button>
              <span v-else class="size-8 shrink-0" aria-hidden="true" />
            </div>
          </div>

          <div class="flex flex-wrap gap-2">
            <Button
              type="button"
              :disabled="!canDownload"
              @click="download('docx')"
            >
              {{ exporting === 'docx' ? 'Préparation…' : 'Word (.docx)' }}
            </Button>
            <Button
              type="button"
              variant="outline"
              :disabled="!canDownload"
              @click="download('pdf')"
            >
              {{ exporting === 'pdf' ? 'Préparation…' : 'PDF' }}
            </Button>
          </div>
        </template>
      </CardContent>
    </Card>
  </div>
</template>
