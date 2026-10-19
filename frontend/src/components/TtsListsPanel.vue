<script setup lang="ts">
import { computed, ref } from 'vue'
import { toast } from 'vue-sonner'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import PlayerPicker from '@/components/PlayerPicker.vue'
import { expandTtsList } from '@/lib/api'
import { normalizeArmyListCode } from '@/lib/armyList'
import { withBase } from '@/lib/basePath'
import type { TtsListExpandEntry, TtsListExpandResult, TtsListRankedFigurine } from '@/types/elo'

defineProps<{
  painterOptions: { value: string; label: string }[]
}>()

const emit = defineEmits<{
  'update:armyId': [value: string]
}>()

const code = ref('')
const preferredPainter = ref('')
const loading = ref(false)
const error = ref('')
const result = ref<TtsListExpandResult | null>(null)
/** index d'entrée → id figurine sélectionnée */
const picks = ref<Record<number, number>>({})

const unresolvedCount = computed(
  () => result.value?.entries.filter((entry) => !entry.profile).length ?? 0,
)

const missingFigCount = computed(
  () => result.value?.entries.filter((entry) => entry.profile && entry.figurines.length === 0).length ?? 0,
)

function photoSrc(figurine: TtsListRankedFigurine) {
  if (!figurine.photo_url) return ''
  return withBase(`${figurine.photo_url}?thumb=1&v=${figurine.updated_at}`)
}

function pickFor(entry: TtsListExpandEntry) {
  const id = picks.value[entry.index]
  return entry.figurines.find((figurine) => figurine.id === id) ?? entry.figurines[0] ?? null
}

function selectFigurine(entryIndex: number, figurineId: number) {
  picks.value = { ...picks.value, [entryIndex]: figurineId }
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
  try {
    const expanded = await expandTtsList(normalized, preferredPainter.value || undefined)
    result.value = expanded
    emit('update:armyId', String(expanded.army_id))
    const next: Record<number, number> = {}
    for (const entry of expanded.entries) {
      const first = entry.figurines[0]
      if (first) next[entry.index] = first.id
    }
    picks.value = next
    if (expanded.entries.length === 0) {
      toast.message('Liste vide')
    } else {
      toast.success(`${expanded.entries.length} entrée${expanded.entries.length > 1 ? 's' : ''} décodée${expanded.entries.length > 1 ? 's' : ''}`)
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
                <template v-if="entry.profile">
                  {{ entry.profile.isc }}
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
                <span v-if="entry.reinforcements"> · renforts</span>
                <span v-if="entry.customization_json"> · custom</span>
                <span v-if="!entry.profile" class="text-destructive"> · profil absent</span>
              </p>
            </div>
            <p v-if="pickFor(entry)" class="text-muted-foreground text-xs">
              {{ pickFor(entry)?.painter || 'Sans peintre' }}
              <span v-if="pickFor(entry)?.exact_loadout"> · loadout exact</span>
            </p>
          </div>

          <div v-if="entry.figurines.length" class="mt-3 flex flex-wrap gap-2">
            <button
              v-for="figurine in entry.figurines"
              :key="figurine.id"
              type="button"
              class="border-border hover:border-foreground/40 focus-visible:ring-ring flex w-28 flex-col overflow-hidden rounded-md border text-left transition focus-visible:outline-none focus-visible:ring-2"
              :class="pickFor(entry)?.id === figurine.id ? 'border-foreground ring-1 ring-foreground' : ''"
              @click="selectFigurine(entry.index, figurine.id)"
            >
              <div class="bg-muted aspect-square w-full overflow-hidden">
                <img
                  v-if="photoSrc(figurine)"
                  :src="photoSrc(figurine)"
                  :alt="figurine.isc"
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
                <p class="truncate text-xs font-medium">{{ figurine.painter || '—' }}</p>
                <p class="text-muted-foreground truncate text-[11px]">{{ figurine.loadout || '—' }}</p>
              </div>
            </button>
          </div>
          <p v-else-if="entry.profile" class="text-muted-foreground mt-2 text-sm">
            Aucune figurine pour cet ISC.
          </p>
        </li>
      </ul>
    </template>
  </div>
</template>
