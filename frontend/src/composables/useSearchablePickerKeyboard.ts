import { nextTick, ref, watch, type ComputedRef, type Ref } from 'vue'

export function useSearchablePickerKeyboard<T>({
  open,
  items,
  disabled,
  onSelect,
  onClose,
  onOpen,
}: {
  open: Ref<boolean>
  items: ComputedRef<T[]>
  disabled: Ref<boolean> | ComputedRef<boolean>
  onSelect: (item: T) => void
  onClose: () => void
  onOpen: () => void
}) {
  const highlightedIndex = ref(-1)
  const optionRefs = ref<(HTMLElement | null)[]>([])

  watch(items, () => {
    if (!open.value) return
    highlightedIndex.value = items.value.length > 0 ? 0 : -1
  })

  watch(open, (isOpen) => {
    highlightedIndex.value = isOpen && items.value.length > 0 ? 0 : -1
    if (!isOpen) {
      optionRefs.value = []
    }
  })

  function setOptionRef(element: HTMLElement | null, index: number) {
    optionRefs.value[index] = element
  }

  function isHighlighted(index: number) {
    return open.value && highlightedIndex.value === index
  }

  function scrollHighlightedIntoView() {
    nextTick(() => {
      optionRefs.value[highlightedIndex.value]?.scrollIntoView({ block: 'nearest' })
    })
  }

  function moveHighlight(delta: number) {
    const count = items.value.length
    if (count === 0) {
      highlightedIndex.value = -1
      return
    }

    if (highlightedIndex.value < 0) {
      highlightedIndex.value = delta > 0 ? 0 : count - 1
    } else {
      highlightedIndex.value =
        (highlightedIndex.value + delta + count) % count
    }

    scrollHighlightedIntoView()
  }

  function handleKeydown(event: KeyboardEvent) {
    if (disabled.value) return

    if (event.key === 'ArrowDown') {
      event.preventDefault()
      if (!open.value) {
        onOpen()
        return
      }
      moveHighlight(1)
      return
    }

    if (event.key === 'ArrowUp') {
      event.preventDefault()
      if (!open.value) {
        onOpen()
        return
      }
      moveHighlight(-1)
      return
    }

    if (event.key === 'Enter') {
      if (!open.value || highlightedIndex.value < 0) return
      const item = items.value[highlightedIndex.value]
      if (!item) return
      event.preventDefault()
      onSelect(item)
      return
    }

    if (event.key === 'Escape') {
      if (!open.value) return
      event.preventDefault()
      onClose()
      return
    }

    if (event.key === 'Tab') {
      if (!open.value) return
      onClose()
    }
  }

  function handleBlur(
    trigger: Ref<HTMLElement | null>,
    dropdown: Ref<HTMLElement | null>,
  ) {
    window.setTimeout(() => {
      const active = document.activeElement
      if (trigger.value?.contains(active) || dropdown.value?.contains(active)) {
        return
      }
      onClose()
    }, 0)
  }

  return {
    handleKeydown,
    handleBlur,
    isHighlighted,
    setOptionRef,
  }
}
