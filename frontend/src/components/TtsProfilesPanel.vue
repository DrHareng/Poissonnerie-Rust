<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import SectorialPicker from '@/components/SectorialPicker.vue'
import ProfileTraitBlock from '@/components/ProfileTraitBlock.vue'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { useArmies } from '@/composables/useArmies'
import { useAppSidePanel } from '@/composables/useAppSidePanel'
import { silhouetteKind } from '@/lib/silhouetteKind'
import {
  fetchTtsProfile,
  fetchTtsProfileArmies,
  fetchTtsProfiles,
  updateTtsProfile,
} from '@/lib/api'
import type { Army, TtsProfileDetail, TtsProfileForm, TtsProfileSummary } from '@/types/elo'
import { isListableArmy } from '@/lib/army'

const { armies, ensureLoaded } = useArmies()
const { setCustomSide } = useAppSidePanel()
const { canEditContent } = useAdminEditMode()

const selectedArmyId = ref<string>()
const showSpecialOps = ref(false)
const search = ref('')
const profiles = ref<TtsProfileSummary[]>([])
const loading = ref(false)
const loadError = ref('')
const detail = ref<TtsProfileDetail | null>(null)
const draft = ref<TtsProfileDetail | null>(null)
const draftSectorialIds = ref<number[]>([])
const addArmyId = ref<string>()
const saving = ref(false)
const extraArmies = ref<Army[]>([])

const pickerArmies = computed(() => {
  const known = new Set(armies.value.map((army) => army.id))
  return [...armies.value, ...extraArmies.value.filter((army) => !known.has(army.id))]
    .filter((army) => isListableArmy(army))
})

function isSpecialOps(profile: { isc: string }) {
  const name = profile.isc.toLowerCase()
  return name.includes('team-ops') || name.includes('team ops') || name.includes('spec-ops') || name.includes('spec ops')
}

const filteredProfiles = computed(() => {
  const needle = search.value.trim().toLowerCase()
  return profiles.value.filter((profile) => {
    if (!showSpecialOps.value && isSpecialOps(profile)) return false
    if (!needle) return true
    return [profile.isc, profile.loadout, profile.nom_tts, profile.marker_name ?? '', profile.differences]
      .join(' ')
      .toLowerCase()
      .includes(needle)
  })
})

const tableRows = computed(() => {
  const rows = filteredProfiles.value.map((profile) => ({
    profile,
    showIsc: true,
    iscSpan: 1,
  }))
  let index = 0
  while (index < rows.length) {
    let end = index + 1
    while (end < rows.length && sameIsc(rows[index].profile, rows[end].profile)) end += 1
    rows[index].iscSpan = end - index
    for (let cursor = index + 1; cursor < end; cursor += 1) rows[cursor].showIsc = false
    index = end
  }
  return rows
})

function sameIsc(left: TtsProfileSummary, right: TtsProfileSummary) {
  return left.isc.localeCompare(right.isc, undefined, { sensitivity: 'base' }) === 0
}

onMounted(async () => {
  try {
    await ensureLoaded()
    const counts = await fetchTtsProfileArmies()
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
  } catch (error) {
    loadError.value = error instanceof Error ? error.message : 'Impossible de charger les sectorielles'
  }
})

watch(selectedArmyId, () => {
  search.value = ''
  void loadProfiles()
})

async function loadProfiles(keepId?: number) {
  if (keepId == null) {
    detail.value = null
    draft.value = null
  }
  if (!selectedArmyId.value) {
    profiles.value = []
    return
  }
  loading.value = true
  loadError.value = ''
  try {
    profiles.value = await fetchTtsProfiles({
      armyId: Number(selectedArmyId.value),
      includeFaction: false,
    })
  } catch (error) {
    profiles.value = []
    loadError.value = error instanceof Error ? error.message : 'Impossible de charger les profils'
  } finally {
    loading.value = false
  }
}

function silhouetteLabel(profile: { silhouette_size: string | null; silhouette_type: string | null }) {
  const size = profile.silhouette_size ? `S${profile.silhouette_size}` : ''
  return [size, profile.silhouette_type || 'normale'].filter(Boolean).join(' · ')
}

async function openProfile(id: number) {
  loadError.value = ''
  try {
    const profile = await fetchTtsProfile(id)
    detail.value = profile
    draft.value = structuredClone(profile)
    draftSectorialIds.value = [...profile.sectorial_ids]
    addArmyId.value = undefined
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible d’ouvrir le profil')
  }
}

function emptyToNull(value: string) {
  const trimmed = value.trim()
  return trimmed ? trimmed : null
}

async function saveProfile() {
  if (!canEditContent.value || !draft.value) return
  saving.value = true
  try {
    const payload = {
      army_id: draft.value.army_id,
      isc: draft.value.isc,
      loadout: draft.value.loadout,
      nom_tts: draft.value.nom_tts,
      description_tts: draft.value.description_tts,
      silhouette_size: emptyToNull(draft.value.silhouette_size ?? ''),
      marker_name: emptyToNull(draft.value.marker_name ?? ''),
      sectorial_ids: draftSectorialIds.value,
      forms: draft.value.forms.map((form) => ({
        id: form.id,
        isc: form.isc,
        loadout: form.loadout,
        nom_tts: form.nom_tts,
        description_tts: form.description_tts,
        silhouette_size: emptyToNull(form.silhouette_size ?? ''),
        marker_name: emptyToNull(form.marker_name ?? ''),
      })),
    }
    const updated = await updateTtsProfile(draft.value.id, payload)
    toast.success('Profil enregistré')
    draftSectorialIds.value = [...updated.sectorial_ids]
    await loadProfiles(updated.id)
    if (profiles.value.some((profile) => profile.id === updated.id)) {
      detail.value = updated
      draft.value = structuredClone(updated)
    } else {
      detail.value = null
      draft.value = null
    }
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Enregistrement impossible')
  } finally {
    saving.value = false
  }
}

function formTitle(form: TtsProfileForm) {
  if (form.nom_tts.toLowerCase().includes('(sapper)')) return `${form.isc} (sapper)`
  return form.isc || form.nom_tts
}

function armyName(id: number) {
  return pickerArmies.value.find((army) => army.id === id)?.name ?? `Sectorielle ${id}`
}

function addSectorial() {
  if (!canEditContent.value) return
  const id = Number(addArmyId.value)
  if (!id || draftSectorialIds.value.includes(id)) return
  draftSectorialIds.value = [...draftSectorialIds.value, id]
  addArmyId.value = undefined
}

function removeSectorial(id: number) {
  if (!canEditContent.value) return
  draftSectorialIds.value = draftSectorialIds.value.filter((armyId) => armyId !== id)
}

function closeSheet() {
  detail.value = null
  draft.value = null
}

watch(detail, (profile) => setCustomSide(profile != null), { immediate: true })
onBeforeUnmount(() => setCustomSide(false))
</script>

<template>
  <Teleport defer to="#app-side-panel">
    <Card v-if="detail" class="neon-panel flex h-full max-h-full min-h-0 flex-col overflow-hidden">
      <CardHeader class="shrink-0 pb-3">
        <div class="flex items-start justify-between gap-2">
          <div class="min-w-0">
            <CardTitle class="break-words">{{ detail.isc }}</CardTitle>
            <CardDescription class="break-words">{{ detail.loadout || '—' }}</CardDescription>
          </div>
          <Button type="button" variant="ghost" size="sm" @click="closeSheet">Fermer</Button>
        </div>
      </CardHeader>
      <CardContent class="min-h-0 flex-1 space-y-4 overflow-y-auto pt-0 text-sm">
        <p>{{ silhouetteLabel(detail) }}</p>
        <p v-if="detail.sectorial_ids.length">
          <span class="text-muted-foreground">Sectorielles</span>
          {{ detail.sectorial_ids.map(armyName).join(', ') }}
        </p>
        <ProfileTraitBlock :traits="detail.traits" />
        <ProfileTraitBlock
          v-for="form in detail.forms"
          :key="form.id"
          class="border-t border-border pt-3"
          :traits="form.traits"
          :heading="form.isc"
          :subtitle="`${form.loadout || '—'} · ${silhouetteLabel(form)}`"
        />
      </CardContent>
    </Card>
  </Teleport>
  <div class="grid gap-4">
    <div class="flex flex-col gap-3 lg:flex-row lg:items-end">
      <div class="grid min-w-0 flex-1 gap-1.5">
        <Label>Sectorielle</Label>
        <SectorialPicker
          v-model="selectedArmyId"
          :armies="pickerArmies"
          :clearable="false"
          placeholder="Choisir une sectorielle"
        />
      </div>
      <div v-if="selectedArmyId" class="grid min-w-0 flex-1 gap-1.5">
        <Label for="tts-profile-search">Recherche</Label>
        <Input
          id="tts-profile-search"
          v-model="search"
          placeholder="ISC, loadout, nom…"
        />
      </div>
    </div>

    <label
      v-if="selectedArmyId"
      class="flex items-start gap-2 text-sm text-muted-foreground"
    >
      <input v-model="showSpecialOps" type="checkbox" class="mt-1" />
      <span>Voir les team-ops/spec-ops</span>
    </label>

    <Alert v-if="loadError" variant="destructive" class="neon-panel-accent">
      <AlertTitle>Chargement impossible</AlertTitle>
      <AlertDescription>{{ loadError }}</AlertDescription>
    </Alert>

    <p v-else-if="!selectedArmyId" class="text-sm text-muted-foreground">
      Choisis une sectorielle pour afficher ses profils.
    </p>
    <p v-else-if="loading" class="text-sm text-muted-foreground">
      Chargement…
    </p>
    <p v-else-if="filteredProfiles.length === 0" class="text-sm text-muted-foreground">
      Aucun profil.
    </p>

    <div v-else class="profile-table max-h-[32rem] overflow-auto rounded-lg border border-border">
      <Table class="table-fixed border-separate border-spacing-0">
        <colgroup>
          <col style="width: 25%" />
          <col style="width: 75%" />
        </colgroup>
        <TableHeader>
          <TableRow>
            <TableHead class="sticky top-0 z-20 w-1/4 border-b border-border bg-background whitespace-normal">ISC</TableHead>
            <TableHead class="sticky top-0 z-20 w-3/4 border-b border-border bg-background whitespace-normal">Loadout</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow
            v-for="row in tableRows"
            :key="row.profile.id"
            class="cursor-pointer"
            :class="detail?.id === row.profile.id ? 'bg-primary/10' : undefined"
            @click="openProfile(row.profile.id)"
          >
            <TableCell
              v-if="row.showIsc"
              :rowspan="row.iscSpan"
              class="w-1/4 border-b border-border bg-background align-top font-medium whitespace-normal"
            >
              <div class="sticky top-10 z-10 bg-background">
                {{ row.profile.isc }}
              </div>
            </TableCell>
            <TableCell class="w-3/4 border-b border-border whitespace-normal">
              {{ row.profile.loadout || '—' }}
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </div>

    <p v-if="!loading && selectedArmyId" class="text-xs text-muted-foreground">
      {{ filteredProfiles.length }} profil{{ filteredProfiles.length > 1 ? 's' : '' }}
    </p>

    <form
      v-if="draft"
      class="grid gap-4 rounded-lg border border-border p-4"
      @submit.prevent="saveProfile"
    >
      <fieldset :disabled="!canEditContent" class="m-0 grid min-w-0 gap-4 border-0 p-0">
      <div class="grid gap-2">
        <Label>Sectorielles associées</Label>
        <p class="text-xs text-muted-foreground">
          Catalogue d’origine : {{ draft.faction_label || 'sans faction' }}.
          <template v-if="canEditContent">Tu peux lier ce profil à plusieurs sectorielles.</template>
        </p>
        <div v-if="draftSectorialIds.length" class="flex flex-wrap gap-2">
          <span
            v-for="id in draftSectorialIds"
            :key="id"
            class="inline-flex items-center gap-1 rounded-full border border-border px-2 py-0.5 text-sm"
          >
            {{ armyName(id) }}
            <button
              v-if="canEditContent"
              type="button"
              class="text-muted-foreground"
              @click="removeSectorial(id)"
            >×</button>
          </span>
        </div>
        <p v-else class="text-sm text-muted-foreground">Aucune sectorielle associée.</p>
        <div v-if="canEditContent" class="flex flex-col gap-2 sm:flex-row sm:items-end">
          <div class="min-w-0 flex-1">
            <SectorialPicker
              v-model="addArmyId"
              :armies="pickerArmies"
              placeholder="Ajouter une sectorielle"
            />
          </div>
          <Button type="button" variant="outline" @click="addSectorial">Ajouter</Button>
        </div>
      </div>

      <div class="grid items-start gap-4" :class="draft.forms.length ? 'xl:grid-cols-2' : ''">
        <article class="grid gap-3 rounded-lg border border-border p-3">
          <h2 class="text-sm font-medium">{{ draft.isc }}</h2>
          <div class="grid gap-3 md:grid-cols-2">
            <div class="grid gap-1.5">
              <Label for="profile-isc">ISC</Label>
              <Input id="profile-isc" v-model="draft.isc" required maxlength="255" />
            </div>
            <div class="grid gap-1.5">
              <Label for="profile-loadout">Loadout</Label>
              <Input id="profile-loadout" v-model="draft.loadout" />
            </div>
          </div>
          <div class="grid gap-1.5">
            <Label for="profile-name">Nom TTS</Label>
            <textarea
              id="profile-name"
              v-model="draft.nom_tts"
              rows="2"
              required
              class="border-input bg-transparent focus-visible:border-ring focus-visible:ring-ring/50 w-full rounded-lg border px-2.5 py-1.5 text-sm outline-none focus-visible:ring-3"
            />
          </div>
          <div class="grid gap-1.5">
            <Label for="profile-description">Description TTS</Label>
            <textarea
              id="profile-description"
              v-model="draft.description_tts"
              rows="8"
              class="border-input bg-transparent focus-visible:border-ring focus-visible:ring-ring/50 w-full rounded-lg border px-2.5 py-1.5 font-mono text-xs outline-none focus-visible:ring-3"
            />
          </div>
          <div class="grid gap-3 md:grid-cols-3">
            <div class="grid gap-1.5">
              <Label for="profile-size">Taille</Label>
              <Input
                id="profile-size"
                :model-value="draft.silhouette_size ?? ''"
                @update:model-value="draft.silhouette_size = String($event)"
              />
            </div>
            <div class="grid gap-1.5">
              <Label>Type</Label>
              <p class="px-1 py-1.5 text-sm">{{ silhouetteKind(draft.description_tts) }}</p>
            </div>
            <div class="grid gap-1.5">
              <Label for="profile-marker">Marqueur</Label>
              <Input
                id="profile-marker"
                :model-value="draft.marker_name ?? ''"
                @update:model-value="draft.marker_name = String($event)"
              />
            </div>
          </div>
        </article>

        <article
          v-for="form in draft.forms"
          :key="form.id"
          class="grid gap-3 rounded-lg border border-border p-3"
        >
          <h2 class="text-sm font-medium">{{ formTitle(form) }}</h2>
          <div class="grid gap-3 md:grid-cols-2">
            <div class="grid gap-1.5">
              <Label :for="`form-isc-${form.id}`">ISC</Label>
              <Input :id="`form-isc-${form.id}`" v-model="form.isc" required maxlength="255" />
            </div>
            <div class="grid gap-1.5">
              <Label :for="`form-loadout-${form.id}`">Loadout</Label>
              <Input :id="`form-loadout-${form.id}`" v-model="form.loadout" />
            </div>
          </div>
          <div class="grid gap-1.5">
            <Label :for="`form-name-${form.id}`">Nom TTS</Label>
            <textarea
              :id="`form-name-${form.id}`"
              v-model="form.nom_tts"
              rows="2"
              required
              class="border-input bg-transparent focus-visible:border-ring focus-visible:ring-ring/50 w-full rounded-lg border px-2.5 py-1.5 text-sm outline-none focus-visible:ring-3"
            />
          </div>
          <div class="grid gap-1.5">
            <Label :for="`form-description-${form.id}`">Description TTS</Label>
            <textarea
              :id="`form-description-${form.id}`"
              v-model="form.description_tts"
              rows="8"
              class="border-input bg-transparent focus-visible:border-ring focus-visible:ring-ring/50 w-full rounded-lg border px-2.5 py-1.5 font-mono text-xs outline-none focus-visible:ring-3"
            />
          </div>
          <div class="grid gap-3 md:grid-cols-3">
            <div class="grid gap-1.5">
              <Label :for="`form-size-${form.id}`">Taille</Label>
              <Input
                :id="`form-size-${form.id}`"
                :model-value="form.silhouette_size ?? ''"
                @update:model-value="form.silhouette_size = String($event)"
              />
            </div>
            <div class="grid gap-1.5">
              <Label>Type</Label>
              <p class="px-1 py-1.5 text-sm">{{ silhouetteKind(form.description_tts) }}</p>
            </div>
            <div class="grid gap-1.5">
              <Label :for="`form-marker-${form.id}`">Marqueur</Label>
              <Input
                :id="`form-marker-${form.id}`"
                :model-value="form.marker_name ?? ''"
                @update:model-value="form.marker_name = String($event)"
              />
            </div>
          </div>
        </article>
      </div>

      </fieldset>
      <div v-if="canEditContent">
        <Button type="submit" :disabled="saving">
          {{ saving ? 'Enregistrement…' : 'Enregistrer' }}
        </Button>
      </div>
    </form>
  </div>
</template>

<style scoped>
.profile-table :deep([data-slot="table-container"]) {
  overflow: visible;
}
</style>
