<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import PlayerPicker from '@/components/PlayerPicker.vue'
import SectorialPicker from '@/components/SectorialPicker.vue'
import TtsFigurinesPanel from '@/components/TtsFigurinesPanel.vue'
import TtsProfilesPanel from '@/components/TtsProfilesPanel.vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { useArmies } from '@/composables/useArmies'
import { useTtsSectorial } from '@/composables/useTtsSectorial'
import { fetchTtsFigurines, fetchTtsProfileArmies } from '@/lib/api'
import { isListableArmy } from '@/lib/army'
import { profilsFigsTabs } from '@/lib/pageTitleTabs'
import type { Army } from '@/types/elo'

const route = useRoute()
const { armies, ensureLoaded } = useArmies()
const { canEditContent } = useAdminEditMode()
const { armyId, ensureLoaded: ensureSectorial, setArmyId } = useTtsSectorial()
const extraArmies = ref<Army[]>([])
const pickerError = ref('')
const search = ref('')
const painterFilter = ref('')
const painterOptions = ref<{ value: string; label: string }[]>([])
const figsPanel = ref<{ openCreate: () => void } | null>(null)

const isProfilesTab = computed(() => route.name === 'profils-figs-profils')
const isFigsTab = computed(() => route.name === 'profils-figs-figs')

const showSearch = computed(() => {
  if (isProfilesTab.value) return !!armyId.value
  if (!isFigsTab.value) return false
  return !!armyId.value || !!painterFilter.value.trim()
})

const pickerArmies = computed(() => {
  const known = new Set(armies.value.map((army) => army.id))
  return [...armies.value, ...extraArmies.value.filter((army) => !known.has(army.id))]
    .filter((army) => isListableArmy(army))
})

async function refreshPainterOptions() {
  try {
    const rows = await fetchTtsFigurines()
    const seen = new Set<string>()
    const options: { value: string; label: string }[] = []
    for (const row of rows) {
      const painter = row.painter.trim()
      if (!painter) continue
      const key = painter.toLowerCase()
      if (seen.has(key)) continue
      seen.add(key)
      options.push({ value: painter, label: painter })
    }
    options.sort((left, right) => left.label.localeCompare(right.label, 'fr', { sensitivity: 'base' }))
    painterOptions.value = options
  } catch {
    painterOptions.value = []
  }
}

onMounted(async () => {
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
  } catch (error) {
    pickerError.value = error instanceof Error ? error.message : 'Impossible de charger les sectorielles'
  }
})
</script>

<template>
  <div class="page-stack">
    <PageTitleTabs
      ariaLabel="Profils et figurines"
      :tabs="profilsFigsTabs"
    />

    <div class="space-y-1.5">
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

    <TtsProfilesPanel v-if="isProfilesTab" v-model:search="search" :army-id="armyId" />

    <TtsFigurinesPanel
      v-else-if="isFigsTab"
      ref="figsPanel"
      v-model:search="search"
      v-model:painter="painterFilter"
      :army-id="armyId"
      @saved="refreshPainterOptions"
    />

    <p v-else class="text-sm text-muted-foreground">
      Listes TTS — section en cours de préparation.
    </p>
  </div>
</template>
