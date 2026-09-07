import { computed, ref, toValue, watch, type MaybeRefOrGetter } from 'vue'
import { useRoute, useRouter } from 'vue-router'

export const LIST_PAGE_QUERY_KEY = 'page'

function firstQueryValue(raw: unknown): string | null {
  const value = Array.isArray(raw) ? raw[0] : raw
  return typeof value === 'string' ? value : null
}

export function parsePageQuery(raw: unknown): number | null {
  const value = firstQueryValue(raw)
  if (value == null || !/^\d+$/.test(value)) return null
  const parsed = Number.parseInt(value, 10)
  if (!Number.isFinite(parsed) || parsed < 1) return null
  return parsed
}

export function useListPage(options?: {
  enabled?: MaybeRefOrGetter<boolean>
}) {
  const route = useRoute()
  const router = useRouter()
  const enabled = computed(() =>
    options?.enabled == null ? true : Boolean(toValue(options.enabled)),
  )

  function resolvePage(): number {
    if (!enabled.value) return 1
    return parsePageQuery(route.query[LIST_PAGE_QUERY_KEY]) ?? 1
  }

  const page = ref(resolvePage())
  let writingQuery = false

  function persist(next: number) {
    if (!enabled.value) return

    const current = parsePageQuery(route.query[LIST_PAGE_QUERY_KEY])
    const shouldHaveQuery = next > 1
    if (shouldHaveQuery && current === next) return
    if (!shouldHaveQuery && current == null) return

    const query = { ...route.query }
    if (shouldHaveQuery) query[LIST_PAGE_QUERY_KEY] = String(next)
    else delete query[LIST_PAGE_QUERY_KEY]

    writingQuery = true
    void router.replace({ query, hash: route.hash }).finally(() => {
      writingQuery = false
    })
  }

  function setPage(next: number) {
    const clamped = Math.max(1, Math.trunc(next) || 1)
    page.value = clamped
    persist(clamped)
  }

  function clampToTotalPages(totalPages: number) {
    const max = Math.max(1, Math.trunc(totalPages) || 1)
    if (page.value > max) setPage(max)
  }

  watch(
    () => [enabled.value, route.query[LIST_PAGE_QUERY_KEY]] as const,
    () => {
      if (writingQuery) return
      const next = resolvePage()
      if (next !== page.value) page.value = next
      persist(next)
    },
  )

  persist(page.value)

  return { page, setPage, clampToTotalPages }
}
