import { computed, ref, watch } from 'vue'
import { useRoute, type RouteLocationRaw } from 'vue-router'
import { fetchPendingConfirmations, type PendingMatchConfirmation } from '@/lib/api'
import { useAuth } from '@/composables/useAuth'

const items = ref<PendingMatchConfirmation[]>([])
const loading = ref(false)
let requestId = 0

export function pendingConfirmationLabel(count: number): string {
  if (count <= 1) return '1 résultat à confirmer'
  return `${count} résultats à confirmer`
}

export async function refreshPendingConfirmations(): Promise<void> {
  const id = ++requestId
  loading.value = true
  try {
    const next = await fetchPendingConfirmations()
    if (id !== requestId) return
    items.value = next
  } catch {
    if (id !== requestId) return
    items.value = []
  } finally {
    if (id === requestId) loading.value = false
  }
}

export function usePendingConfirmations() {
  const route = useRoute()
  const { isAuthenticated, hasPlayer } = useAuth()

  const count = computed(() => items.value.length)

  const menuLabel = computed(() => pendingConfirmationLabel(count.value))

  const menuRoute = computed((): RouteLocationRaw | null => {
    if (count.value === 0) return null
    const tournaments = new Set(items.value.map((item) => item.tournament_id))
    if (tournaments.size === 1) {
      return {
        name: 'tournoi' as const,
        params: { id: items.value[0]!.tournament_id },
      }
    }
    return { name: 'tournois' as const }
  })

  async function refresh() {
    if (!isAuthenticated.value || !hasPlayer.value) {
      items.value = []
      return
    }
    await refreshPendingConfirmations()
  }

  watch(
    [isAuthenticated, hasPlayer],
    ([authenticated, player]) => {
      if (authenticated && player) void refresh()
      else items.value = []
    },
    { immediate: true },
  )

  watch(
    () => route.fullPath,
    () => {
      if (isAuthenticated.value && hasPlayer.value) void refresh()
    },
  )

  return {
    items: computed(() => items.value),
    count,
    menuLabel,
    menuRoute,
    loading: computed(() => loading.value),
    refresh,
  }
}
