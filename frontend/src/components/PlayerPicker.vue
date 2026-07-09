<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { onClickOutside, useElementBounding, useEventListener } from '@vueuse/core'
import { ChevronsUpDown } from '@lucide/vue'
import { Input } from '@/components/ui/input'
import { useSearchablePickerKeyboard } from '@/composables/useSearchablePickerKeyboard'

export interface PlayerPickerOption {
  value: string
  label: string
}

const props = withDefaults(
  defineProps<{
    modelValue?: string
    options: PlayerPickerOption[]
    disabled?: boolean
    placeholder?: string
    emptyMessage?: string
  }>(),
  {
    placeholder: 'Tapez pour chercher un joueur',
    emptyMessage: 'Aucun joueur trouvé.',
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

const inputValue = computed({
  get() {
    if (open.value) {
      return query.value
    }
    return selectedOption.value?.label ?? ''
  },
  set(value: string | number) {
    query.value = String(value)
    onInput()
  },
})

const { handleKeydown, handleBlur, isHighlighted, setOptionRef } = useSearchablePickerKeyboard({
  open,
  items: filteredOptions,
  disabled: computed(() => !!props.disabled),
  onSelect: selectOption,
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
  query.value = selectedOption.value?.label ?? ''
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

function selectOption(option: PlayerPickerOption) {
  emit('update:modelValue', option.value)
  query.value = option.label
  open.value = false
}

function onInput() {
  if (!open.value) {
    open.value = true
  }

  if (selectedOption.value && query.value !== selectedOption.value.label) {
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
          v-if="filteredOptions.length === 0"
          class="px-3 py-2 text-sm text-muted-foreground"
        >
          {{ emptyMessage }}
        </p>

        <button
          v-for="(option, index) in filteredOptions"
          :key="option.value"
          :ref="(element) => setOptionRef(element as HTMLElement | null, index)"
          type="button"
          class="searchable-picker-option"
          :class="{
            'searchable-picker-option-active': option.value === modelValue && !isHighlighted(index),
            'searchable-picker-option-highlighted': isHighlighted(index),
          }"
          @click="selectOption(option)"
        >
          <span class="truncate">{{ option.label }}</span>
        </button>
      </div>
    </Teleport>
  </div>
</template>
