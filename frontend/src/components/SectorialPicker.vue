<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { onClickOutside, useElementBounding, useEventListener } from '@vueuse/core'
import { ChevronsUpDown } from '@lucide/vue'
import type { Army } from '@/types/elo'
import { Input } from '@/components/ui/input'
import { useSearchablePickerKeyboard } from '@/composables/useSearchablePickerKeyboard'

type PickerItem =
  | { kind: 'empty' }
  | { kind: 'army'; army: Army }

const props = withDefaults(
  defineProps<{
    modelValue?: string
    armies: Army[]
    disabled?: boolean
    placeholder?: string
    /** Affiche une option pour vider la sélection (ex. « Toutes »). */
    allowEmpty?: boolean
    emptyLabel?: string
  }>(),
  {
    allowEmpty: false,
    emptyLabel: 'Toutes',
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: string | undefined]
}>()

const trigger = ref<HTMLElement | null>(null)
const dropdown = ref<HTMLElement | null>(null)
const open = ref(false)
const query = ref('')

const { top, left, width, height, update } = useElementBounding(trigger)

const dropdownStyle = computed(() => ({
  top: `${top.value + height.value + 4}px`,
  left: `${left.value}px`,
  width: `${width.value}px`,
}))

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

const pickerItems = computed<PickerItem[]>(() => {
  const items: PickerItem[] = []
  if (props.allowEmpty) {
    const needle = query.value.trim().toLowerCase()
    if (!needle || props.emptyLabel.toLowerCase().includes(needle)) {
      items.push({ kind: 'empty' })
    }
  }
  for (const army of filteredArmies.value) {
    items.push({ kind: 'army', army })
  }
  return items
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

const { handleKeydown, handleBlur, isHighlighted, setOptionRef } = useSearchablePickerKeyboard({
  open,
  items: pickerItems,
  disabled: computed(() => !!props.disabled),
  onSelect: selectItem,
  onClose: closePicker,
  onOpen: openPicker,
})

onClickOutside(
  trigger,
  () => {
    closePicker()
  },
  { ignore: [dropdown] },
)

useEventListener('scroll', () => {
  if (open.value) update()
}, true)

useEventListener('resize', () => {
  if (open.value) update()
})

watch(
  () => props.modelValue,
  () => {
    syncQueryWithSelection()
  },
)

watch(open, async (isOpen) => {
  if (isOpen) {
    await nextTick()
    update()
  }
})

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

function closePicker() {
  open.value = false
  syncQueryWithSelection()
}

function selectItem(item: PickerItem) {
  if (item.kind === 'empty') {
    emit('update:modelValue', undefined)
    query.value = ''
    open.value = false
    return
  }
  emit('update:modelValue', String(item.army.id))
  query.value = item.army.name
  open.value = false
}

function onInput() {
  if (!open.value) {
    open.value = true
  }

  // Avec allowEmpty, on ne vide la sélection que via l'option dédiée
  // (évite de tout recharger pendant la saisie).
  if (props.allowEmpty) {
    return
  }

  if (selectedArmy.value && query.value !== selectedArmy.value.name) {
    emit('update:modelValue', undefined)
  }
}
</script>

<template>
  <div class="searchable-picker">
    <div
      ref="trigger"
      class="searchable-picker-trigger"
      :class="{ 'searchable-picker-trigger-disabled': disabled }"
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
        class="searchable-picker-input"
        @focus="openPicker"
        @blur="handleBlur(trigger, dropdown)"
        @keydown="handleKeydown"
      />

      <button
        type="button"
        class="searchable-picker-toggle"
        :disabled="disabled"
        tabindex="-1"
        @click="openPicker"
      >
        <ChevronsUpDown class="size-4 opacity-60" />
      </button>
    </div>

    <Teleport to="body">
      <div
        v-if="open && !disabled"
        ref="dropdown"
        class="searchable-picker-dropdown"
        :style="dropdownStyle"
        @mousedown.prevent
      >
        <p
          v-if="pickerItems.length === 0"
          class="px-3 py-2 text-sm text-muted-foreground"
        >
          Aucune sectorielle trouvée.
        </p>

        <button
          v-for="(item, index) in pickerItems"
          :key="item.kind === 'empty' ? 'empty' : item.army.id"
          :ref="(element) => setOptionRef(element as HTMLElement | null, index)"
          type="button"
          class="searchable-picker-option"
          :class="{
            'searchable-picker-option-active':
              (item.kind === 'empty'
                ? modelValue == null || modelValue === ''
                : String(item.army.id) === modelValue) && !isHighlighted(index),
            'searchable-picker-option-highlighted': isHighlighted(index),
          }"
          @click="selectItem(item)"
        >
          <template v-if="item.kind === 'empty'">
            <span class="truncate text-muted-foreground">{{ emptyLabel }}</span>
          </template>
          <template v-else>
            <img :src="item.army.logo_url" :alt="item.army.name" class="army-logo shrink-0" />
            <span class="truncate">{{ item.army.name }}</span>
          </template>
        </button>
      </div>
    </Teleport>
  </div>
</template>
