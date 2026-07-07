<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { onClickOutside } from '@vueuse/core'
import { ChevronsUpDown } from '@lucide/vue'
import type { Army } from '@/types/elo'
import { Input } from '@/components/ui/input'

const props = defineProps<{
  modelValue?: string
  armies: Army[]
  disabled?: boolean
  placeholder?: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string | undefined]
}>()

const root = ref<HTMLElement | null>(null)
const open = ref(false)
const query = ref('')

const selectedArmy = computed(() =>
  props.armies.find((army) => String(army.id) === props.modelValue),
)

const filteredArmies = computed(() => {
  const needle = query.value.trim().toLowerCase()
  if (!needle) {
    return props.armies
  }

  return props.armies.filter((army) =>
    army.name.toLowerCase().includes(needle),
  )
})

const inputValue = computed({
  get() {
    if (open.value) {
      return query.value
    }
    return selectedArmy.value?.name ?? ''
  },
  set(value: string | number) {
    query.value = String(value)
    onInput()
  },
})

onClickOutside(root, () => {
  open.value = false
  syncQueryWithSelection()
})

watch(
  () => props.modelValue,
  () => {
    syncQueryWithSelection()
  },
)

function syncQueryWithSelection() {
  query.value = selectedArmy.value?.name ?? ''
}

function openPicker() {
  if (props.disabled) {
    return
  }
  open.value = true
  query.value = ''
}

function selectArmy(army: Army) {
  emit('update:modelValue', String(army.id))
  query.value = army.name
  open.value = false
}

function onInput() {
  if (!open.value) {
    open.value = true
  }

  if (selectedArmy.value && query.value !== selectedArmy.value.name) {
    emit('update:modelValue', undefined)
  }
}
</script>

<template>
  <div ref="root" class="sectorial-picker">
    <div
      class="sectorial-picker-trigger"
      :class="{ 'sectorial-picker-trigger-disabled': disabled }"
    >
      <img
        v-if="selectedArmy && !open"
        :src="selectedArmy.logo_url"
        :alt="selectedArmy.name"
        class="army-logo shrink-0"
      />

      <Input
        v-model="inputValue"
        :placeholder="placeholder"
        :disabled="disabled"
        class="sectorial-picker-input"
        @focus="openPicker"
      />

      <button
        type="button"
        class="sectorial-picker-toggle"
        :disabled="disabled"
        tabindex="-1"
        @click="openPicker"
      >
        <ChevronsUpDown class="size-4 opacity-60" />
      </button>
    </div>

    <div v-if="open && !disabled" class="sectorial-picker-dropdown">
      <p
        v-if="filteredArmies.length === 0"
        class="px-3 py-2 text-sm text-muted-foreground"
      >
        Aucune sectorielle trouvée.
      </p>

      <button
        v-for="army in filteredArmies"
        :key="army.id"
        type="button"
        class="sectorial-picker-option"
        :class="{ 'sectorial-picker-option-active': String(army.id) === modelValue }"
        @click="selectArmy(army)"
      >
        <img :src="army.logo_url" :alt="army.name" class="army-logo shrink-0" />
        <span class="truncate">{{ army.name }}</span>
      </button>
    </div>
  </div>
</template>
