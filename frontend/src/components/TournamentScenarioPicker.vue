<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Dices, RefreshCw } from '@lucide/vue'
import { fetchScenarioPack } from '@/lib/api'
import { shufflePick } from '@/lib/shufflePick'
import type { ScenarioSummary, TournamentScenarioSlot } from '@/types/elo'
import { DEFAULT_SCENARIO_PACK_SLUG } from '@/types/elo'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

const props = defineProps<{
  title: string
  description?: string
  slots: { key: string; label: string }[]
  values: TournamentScenarioSlot[]
  canEdit: boolean
  saving?: boolean
}>()

const emit = defineEmits<{
  save: [scenarioIds: number[]]
}>()

const scenarios = ref<ScenarioSummary[]>([])
const loading = ref(true)
const localIds = ref<Record<string, string>>({})
const didAutoDraw = ref(false)

const bySlot = computed(() => {
  const map = new Map<string, TournamentScenarioSlot>()
  for (const item of props.values) {
    map.set(item.slot, item)
  }
  return map
})

function syncLocalFromProps() {
  const next: Record<string, string> = {}
  for (const slot of props.slots) {
    const current = bySlot.value.get(slot.key)
    next[slot.key] = current ? String(current.scenario_id) : ''
  }
  localIds.value = next
}

function hasPropValues() {
  return props.values.some((item) => item.scenario_id > 0)
}

function allSlotsEmpty() {
  return props.slots.every((slot) => !localIds.value[slot.key])
}

function drawLocal() {
  if (scenarios.value.length === 0 || props.slots.length === 0) return
  const picks = shufflePick(scenarios.value, props.slots.length)
  const next: Record<string, string> = {}
  for (let i = 0; i < props.slots.length; i++) {
    const slot = props.slots[i]!
    next[slot.key] = picks[i] ? String(picks[i]!.id) : ''
  }
  localIds.value = next
}

function rerollLocal(slotKey: string) {
  const used = new Set(
    props.slots
      .filter((slot) => slot.key !== slotKey)
      .map((slot) => localIds.value[slot.key])
      .filter(Boolean),
  )
  const available = scenarios.value.filter(
    (scenario) => !used.has(String(scenario.id)),
  )
  const pick = shufflePick(available, 1)[0]
  if (!pick) return
  localIds.value = { ...localIds.value, [slotKey]: String(pick.id) }
}

function optionsFor(slotKey: string) {
  const usedElsewhere = new Set(
    props.slots
      .filter((slot) => slot.key !== slotKey)
      .map((slot) => localIds.value[slot.key])
      .filter(Boolean),
  )
  return scenarios.value.filter(
    (scenario) =>
      !usedElsewhere.has(String(scenario.id))
      || String(scenario.id) === localIds.value[slotKey],
  )
}

function scenarioName(id: string) {
  if (!id) return ''
  return scenarios.value.find((item) => String(item.id) === id)?.name ?? ''
}

function onSelect(slot: string, value: string) {
  localIds.value = { ...localIds.value, [slot]: value }
}

function save() {
  const ids = props.slots.map((slot) => Number(localIds.value[slot.key]))
  if (ids.some((id) => !Number.isFinite(id) || id <= 0)) {
    return
  }
  emit('save', ids)
}

function maybeAutoDraw() {
  if (!props.canEdit || loading.value || didAutoDraw.value) return
  if (scenarios.value.length === 0) return
  if (hasPropValues()) {
    didAutoDraw.value = true
    return
  }
  if (!allSlotsEmpty()) {
    didAutoDraw.value = true
    return
  }
  drawLocal()
  didAutoDraw.value = true
}

const canSave = computed(() => {
  if (!props.canEdit) return false
  return props.slots.every((slot) => {
    const id = Number(localIds.value[slot.key])
    return Number.isFinite(id) && id > 0
  })
})

onMounted(async () => {
  loading.value = true
  try {
    const pack = await fetchScenarioPack(DEFAULT_SCENARIO_PACK_SLUG)
    scenarios.value = pack.scenarios
  } catch {
    scenarios.value = []
  } finally {
    loading.value = false
    syncLocalFromProps()
    maybeAutoDraw()
  }
})

watch(
  () => props.values,
  () => {
    if (hasPropValues()) {
      syncLocalFromProps()
      didAutoDraw.value = true
      return
    }
    maybeAutoDraw()
  },
  { deep: true },
)

watch(
  () => props.canEdit,
  () => maybeAutoDraw(),
)
</script>

<template>
  <div class="grid gap-3 rounded-lg border p-3">
    <div class="flex flex-wrap items-start justify-between gap-2">
      <div>
        <h3 class="font-semibold">{{ title }}</h3>
        <p v-if="description" class="text-sm text-muted-foreground">
          {{ description }}
        </p>
      </div>
      <div v-if="canEdit" class="flex flex-wrap gap-2">
        <Button
          size="sm"
          variant="outline"
          :disabled="loading || scenarios.length === 0"
          @click="drawLocal"
        >
          <Dices class="size-4" />
          Tirer au sort
        </Button>
        <Button size="sm" :disabled="!canSave || saving" @click="save">
          Enregistrer
        </Button>
      </div>
    </div>

    <div class="grid gap-2 sm:grid-cols-2">
      <div v-for="slot in slots" :key="slot.key" class="grid gap-1">
        <Label>{{ slot.label }}</Label>
        <div class="flex items-center gap-2">
          <Select
            :model-value="localIds[slot.key] || undefined"
            :disabled="!canEdit || loading"
            @update:model-value="(value) => onSelect(slot.key, String(value ?? ''))"
          >
            <SelectTrigger>
              <SelectValue :placeholder="loading ? 'Chargement…' : 'Scénario'" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="scenario in optionsFor(slot.key)"
                :key="scenario.id"
                :value="String(scenario.id)"
              >
                {{ scenario.name }}
              </SelectItem>
            </SelectContent>
          </Select>
          <Button
            v-if="canEdit"
            size="sm"
            variant="ghost"
            :disabled="loading || scenarios.length === 0"
            :title="`Reroll ${slot.label}`"
            @click="rerollLocal(slot.key)"
          >
            <RefreshCw class="size-4" />
          </Button>
        </div>
        <p
          v-if="scenarioName(localIds[slot.key] ?? '')"
          class="truncate text-xs text-muted-foreground"
        >
          {{ scenarioName(localIds[slot.key] ?? '') }}
        </p>
      </div>
    </div>
  </div>
</template>
