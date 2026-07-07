import { computed, ref } from 'vue'
import { fetchMe, loginWithDiscord, logout as apiLogout } from '@/lib/api'
import type { AuthUser } from '@/types/elo'

const authUser = ref<AuthUser | null>(null)
const loading = ref(true)
const initialized = ref(false)

export async function refreshAuth() {
  loading.value = true
  try {
    authUser.value = await fetchMe()
  } catch {
    authUser.value = null
  } finally {
    loading.value = false
    initialized.value = true
  }
}

export function useAuth() {
  const user = computed(() => authUser.value?.user ?? null)
  const player = computed(() => authUser.value?.player ?? null)

  async function logout() {
    await apiLogout()
    authUser.value = null
  }

  async function refresh() {
    await refreshAuth()
  }

  return {
    authUser: computed(() => authUser.value),
    user,
    player,
    loading: computed(() => loading.value),
    initialized: computed(() => initialized.value),
    isAuthenticated: computed(() => authUser.value !== null),
    isAdmin: computed(() => authUser.value?.user.is_admin ?? false),
    hasPlayer: computed(() => authUser.value?.player != null),
    login: loginWithDiscord,
    logout,
    refresh,
  }
}
