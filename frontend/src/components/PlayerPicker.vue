<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { onClickOutside, useElementBounding, useEventListener } from '@vueuse/core'
import { ChevronsUpDown, X } from '@lucide/vue'
import { Input } from '@/components/ui/input'
import { useSearchablePickerKeyboard } from '@/composables/useSearchablePickerKeyboard'

export interface PlayerPickerOption {
  value: string
  label: string
}

type PickerItem =
  | { kind: 'empty' }
  | { kind: 'option'; option: PlayerPickerOption }

const props = withDefaults(
  defineProps<{
    modelValue?: string
    query?: string
    options: PlayerPickerOption[]
    disabled?: boolean
    placeholder?: string
    emptyMessage?: string
    allowCustom?: boolean
    /** Affiche une option pour vider la sélection (ex. « Tous »). */
    allowEmpty?: boolean
    emptyLabel?: string
  }>(),
  {
    placeholder: 'Tapez pour chercher un joueur',
    emptyMessage: 'Aucun joueur trouvé.',
    allowCustom: false,
    allowEmpty: false,
    emptyLabel: 'Tous',
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: string | undefined]
  'update:query': [value: string]
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

const selectedOption = computed(() =>
  props.options.find((option) => option.value === props.modelValue),
)

const filteredOptions = computed(() => {
  const needle = query.value.trim().toLowerCase()
  if (!needle) {
    return props.options
  }

  return props.options.filter(
    (option) =>
      option.label.toLowerCase().includes(needle)
      || option.value.toLowerCase().includes(needle),
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
  for (const option of filteredOptions.value) {
    items.push({ kind: 'option', option })
  }
  return items
})

const inputValue = computed({
  get() {
    if (open.value) {
      return query.value
    }
    return selectedOption.value?.label ?? props.modelValue ?? ''
  },
  set(value: string | number) {
    query.value = String(value)
    onInput()
  },
})

const canClear = computed(() => {
  if (props.disabled) return false
  if (props.modelValue != null && props.modelValue !== '') return true
  return open.value && query.value.trim() !== ''
})

const { handleKeydown, handleBlur, isHighlighted, highlightedIndex, setOptionRef } =
  useSearchablePickerKeyboard({
    open,
    items: pickerItems,
    disabled: computed(() => !!props.disabled),
    onSelect: selectItem,
    onClose: closePicker,
    onOpen: openPicker,
    autoHighlight: computed(() => !props.allowCustom),
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

watch(
  () => props.options,
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
  query.value =
    selectedOption.value?.label
    ?? (props.allowCustom ? (props.modelValue ?? '') : '')
}

function openPicker() {
  if (props.disabled) {
    return
  }
  open.value = true
  query.value = ''
}

function commitCustomIfNeeded() {
  if (!props.allowCustom) return
  const typed = query.value.trim()
  if (typed && !selectedOption.value) {
    emit('update:modelValue', typed)
    emit('update:query', typed)
  }
}

function closePicker() {
  commitCustomIfNeeded()
  open.value = false
  syncQueryWithSelection()
}

function selectItem(item: PickerItem) {
  if (item.kind === 'empty') {
    emit('update:modelValue', undefined)
    emit('update:query', '')
    query.value = ''
    open.value = false
    return
  }
  selectOption(item.option)
}

function selectOption(option: PlayerPickerOption) {
  emit('update:modelValue', option.value)
  query.value = option.label
  open.value = false
}

function onInput() {
  if (!open.value) {
    open.value = true
  }

  emit('update:query', query.value)

  // Avec allowEmpty, on ne vide la sélection que via l'option dédiée.
  if (props.allowEmpty) {
    return
  }

  if (selectedOption.value && query.value !== selectedOption.value.label) {
    emit('update:modelValue', undefined)
  }
}

function onKeydown(event: KeyboardEvent) {
  if (props.allowCustom && event.key === 'Enter' && open.value) {
    if (highlightedIndex.value < 0) {
      event.preventDefault()
      closePicker()
      return
    }
  }
  handleKeydown(event)
}

function clearSelection() {
  emit('update:modelValue', undefined)
  emit('update:query', '')
  query.value = ''
  open.value = false
}
</script>

<template>
  <div class="searchable-picker">
    <div
      ref="trigger"
      class="searchable-picker-trigger"
      :class="{ 'searchable-picker-trigger-disabled': disabled }"
    >
      <Input
        v-model="inputValue"
        :placeholder="placeholder"
        :disabled="disabled"
        class="searchable-picker-input"
        @focus="openPicker"
        @blur="handleBlur(trigger, dropdown)"
        @keydown="onKeydown"
      />

      <button
        v-if="canClear"
        type="button"
        class="searchable-picker-toggle"
        :disabled="disabled"
        tabindex="-1"
        title="Réinitialiser"
        aria-label="Réinitialiser"
        @mousedown.prevent
        @click="clearSelection"
      >
        <X class="size-4 opacity-60" />
      </button>

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
          {{ emptyMessage }}
        </p>

        <button
          v-for="(item, index) in pickerItems"
          :key="item.kind === 'empty' ? 'empty' : item.option.value"
          :ref="(element) => setOptionRef(element as HTMLElement | null, index)"
          type="button"
          class="searchable-picker-option"
          :class="{
            'searchable-picker-option-active':
              (item.kind === 'empty'
                ? modelValue == null || modelValue === ''
                : item.option.value === modelValue) && !isHighlighted(index),
            'searchable-picker-option-highlighted': isHighlighted(index),
          }"
          @click="selectItem(item)"
        >
          <span
            class="truncate"
            :class="{ 'text-muted-foreground': item.kind === 'empty' }"
          >
            {{ item.kind === 'empty' ? emptyLabel : item.option.label }}
          </span>
        </button>
      </div>
    </Teleport>
  </div>
</template>
