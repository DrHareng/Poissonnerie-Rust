<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import SectorialPicker from '@/components/SectorialPicker.vue'
import TtsFigurinesPanel from '@/components/TtsFigurinesPanel.vue'
import TtsProfilesPanel from '@/components/TtsProfilesPanel.vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { useArmies } from '@/composables/useArmies'
import { useTtsSectorial } from '@/composables/useTtsSectorial'
import { fetchTtsProfileArmies } from '@/lib/api'
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
const figsPanel = ref<{ openCreate: () => void } | null>(null)

const isProfilesTab = computed(() => route.name === 'profils-figs-profils')
const isFigsTab = computed(() => route.name === 'profils-figs-figs')

const pickerArmies = computed(() => {
  const known = new Set(armies.value.map((army) => army.id))
  return [...armies.value, ...extraArmies.value.filter((army) => !known.has(army.id))]
    .filter((army) => isListableArmy(army))
})

onMounted(async () => {
  try {
    await Promise.all([
      ensureLoaded(),
      ensureSectorial(),
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
            :clearable="false"
            placeholder="Choisir une sectorielle"
            @update:model-value="setArmyId"
          />
        </div>
        <Input
          v-if="armyId && (isProfilesTab || isFigsTab)"
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
      :army-id="armyId"
    />

    <p v-else class="text-sm text-muted-foreground">
      Listes TTS — section en cours de préparation.
    </p>
  </div>
</template>
