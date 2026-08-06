import { computed, ref, watch } from 'vue'
import { fetchMyInProgressMatches } from '@/lib/api'
import type { MatchRecord } from '@/types/elo'
import { useAuth } from '@/composables/useAuth'

const allMatches = ref<MatchRecord[]>([])
const loading = ref(false)
let fetchPromise: Promise<void> | null = null

function normalizePlayerName(name: string): string {
  return name.trim().toLowerCase()
}

function matchesForPlayer(
  items: MatchRecord[],
  playerName: string | undefined,
): MatchRecord[] {
  if (!playerName) return items
  const key = normalizePlayerName(playerName)
  return items.filter(
    (match) =>
      normalizePlayerName(match.player1) === key ||
      normalizePlayerName(match.player2) === key,
  )
}

export function inProgressMenuLabel(count: number): string {
  if (count <= 1) return `${count} partie en cours`
  return `${count} parties en cours`
}

export async function refreshMyInProgressMatches(): Promise<void> {
  if (fetchPromise) return fetchPromise

  fetchPromise = (async () => {
    loading.value = true
    try {
      allMatches.value = await fetchMyInProgressMatches()
    } catch {
      allMatches.value = []
    } finally {
      loading.value = false
      fetchPromise = null
    }
  })()

  return fetchPromise
}

export function useMyInProgressMatches() {
  const { isAuthenticated, player } = useAuth()

  const myMatches = computed(() =>
    matchesForPlayer(allMatches.value, player.value?.name),
  )

  const count = computed(() => myMatches.value.length)

  const menuLabel = computed(() => inProgressMenuLabel(count.value))

  const menuRoute = computed(() => {
    if (!isAuthenticated.value) return null
    if (count.value === 1) {
      return {
        name: 'partie-resume' as const,
        params: { id: String(myMatches.value[0]!.id) },
      }
    }
    return { path: '/matchs', hash: '#parties-en-cours' }
  })

  async function refresh() {
    if (!isAuthenticated.value) {
      allMatches.value = []
      return
    }
    await refreshMyInProgressMatches()
  }

  watch(
    [isAuthenticated, () => player.value?.name],
    ([authenticated]) => {
      if (authenticated) {
        void refresh()
      } else {
        allMatches.value = []
      }
    },
    { immediate: true },
  )

  return {
    allMatches: computed(() => allMatches.value),
    myMatches,
    count,
    menuLabel,
    menuRoute,
    loading: computed(() => loading.value),
    refresh,
  }
}
