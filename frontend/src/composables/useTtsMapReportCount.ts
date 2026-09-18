import { computed, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { fetchTtsMapReportCount } from '@/lib/api'
import { useAuth } from '@/composables/useAuth'

const count = ref(0)
const loading = ref(false)
let requestId = 0

export async function refreshTtsMapReportCount(): Promise<void> {
  const id = ++requestId
  loading.value = true
  try {
    const payload = await fetchTtsMapReportCount()
    if (id !== requestId) return
    count.value = payload.count
  } catch {
    if (id !== requestId) return
    count.value = 0
  } finally {
    if (id === requestId) loading.value = false
  }
}

export function useTtsMapReportCount() {
  const route = useRoute()
  const { isAdmin } = useAuth()

  async function refresh() {
    if (!isAdmin.value) {
      count.value = 0
      return
    }
    await refreshTtsMapReportCount()
  }

  watch(
    isAdmin,
    (admin) => {
      if (admin) void refresh()
      else count.value = 0
    },
    { immediate: true },
  )

  watch(
    () => route.fullPath,
    () => {
      if (isAdmin.value) void refresh()
    },
  )

  return {
    count: computed(() => count.value),
    loading: computed(() => loading.value),
    refresh,
  }
}
