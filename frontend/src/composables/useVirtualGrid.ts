import { computed, onMounted, ref, toValue, type MaybeRefOrGetter } from 'vue'
import { useElementSize, useEventListener, useWindowSize } from '@vueuse/core'

const GAP_PX = 12
const OVERSCAN_ROWS = 2

function columnCount(windowWidth: number) {
  if (windowWidth >= 1536) return 4
  if (windowWidth >= 1024) return 3
  return 2
}

export function useVirtualGrid<T>(options: {
  items: MaybeRefOrGetter<T[]>
  scrollEl: MaybeRefOrGetter<HTMLElement | null | undefined>
  gridEl: MaybeRefOrGetter<HTMLElement | null | undefined>
}) {
  const scrollTop = ref(0)
  const { width: gridWidth } = useElementSize(() => toValue(options.gridEl) ?? null)
  const { width: windowWidth } = useWindowSize()

  const cols = computed(() => columnCount(windowWidth.value))
  const tileSize = computed(() => {
    const width = Math.max(1, gridWidth.value)
    const count = cols.value
    return Math.max(1, (width - GAP_PX * (count - 1)) / count)
  })
  const rowHeight = computed(() => tileSize.value + GAP_PX)
  const items = computed(() => toValue(options.items))
  const rowCount = computed(() => Math.ceil(items.value.length / cols.value))

  function gridContentTop() {
    const scroll = toValue(options.scrollEl)
    const grid = toValue(options.gridEl)
    if (!scroll || !grid) return 0
    return (
      grid.getBoundingClientRect().top -
      scroll.getBoundingClientRect().top +
      scroll.scrollTop
    )
  }

  function readScroll() {
    const el = toValue(options.scrollEl)
    scrollTop.value = el?.scrollTop ?? 0
  }

  useEventListener(() => toValue(options.scrollEl), 'scroll', readScroll, {
    passive: true,
  })
  onMounted(readScroll)

  const startRow = computed(() => {
    const y = Math.max(0, scrollTop.value - gridContentTop())
    return Math.max(0, Math.floor(y / rowHeight.value) - OVERSCAN_ROWS)
  })

  const visibleRowCount = computed(() => {
    const scroll = toValue(options.scrollEl)
    const view = scroll?.clientHeight ?? 800
    return Math.ceil(view / rowHeight.value) + OVERSCAN_ROWS * 2
  })

  const paddingTop = computed(() => startRow.value * rowHeight.value)
  const paddingBottom = computed(() => {
    const remaining = Math.max(
      0,
      rowCount.value - startRow.value - visibleRowCount.value,
    )
    return remaining * rowHeight.value
  })

  const visibleItems = computed(() => {
    const start = startRow.value * cols.value
    const end = Math.min(
      items.value.length,
      start + visibleRowCount.value * cols.value,
    )
    return items.value.slice(start, end)
  })

  return {
    cols,
    paddingTop,
    paddingBottom,
    visibleItems,
  }
}
