<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import {
  CircleAlert,
  Eye,
  LogIn,
  LogOut,
  Pencil,
  Play,
  Shield,
  User,
} from '@lucide/vue'
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from 'reka-ui'
import { useAuth } from '@/composables/useAuth'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { useMyInProgressMatches } from '@/composables/useMyInProgressMatches'
import { useTtsMapReportCount } from '@/composables/useTtsMapReportCount'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

const route = useRoute()
const { user, player, isAuthenticated, hasPlayer, loading, login, logout } =
  useAuth()
const { isAdmin, isEditMode, setEditMode } = useAdminEditMode()
const {
  count: inProgressCount,
  menuLabel: inProgressMenuLabel,
  menuRoute: inProgressRoute,
} = useMyInProgressMatches()
const { count: ttsReportCount } = useTtsMapReportCount()
const reportsRoute = { name: 'admin-tts-reports' as const }

const profileBadgeCount = computed(
  () => inProgressCount.value + ttsReportCount.value,
)

const partieCta = computed(() => {
  if (inProgressCount.value >= 1 && inProgressRoute.value) {
    const several = inProgressCount.value > 1
    return {
      to: inProgressRoute.value,
      label: several ? `Reprendre (${inProgressCount.value})` : 'Reprendre',
      ariaLabel: several
        ? `Reprendre (${inProgressCount.value} parties en cours)`
        : 'Reprendre la partie',
    }
  }
  return {
    to: '/partie',
    label: 'Démarrer une partie',
    ariaLabel: 'Démarrer une partie',
  }
})

const isPartieCtaActive = computed(() => route.path.startsWith('/partie'))

const profileRoute = computed(() =>
  hasPlayer.value && player.value
    ? { name: 'joueur' as const, params: { name: player.value.name } }
    : null,
)

const menuItemClass = cn(
  'relative flex w-full cursor-default select-none items-center gap-2 rounded-md px-2 py-1.5 text-sm outline-none',
  'transition-colors hover:bg-primary/10 hover:text-primary focus:bg-primary/10 focus:text-primary',
)

async function handleLogout() {
  await logout()
}
</script>

<template>
  <nav class="bottom-bar" aria-label="Actions rapides">
    <div class="bottom-bar-inner">
      <div class="bottom-bar-auth">
        <div v-if="loading" class="text-xs text-muted-foreground">…</div>
        <template v-else-if="isAuthenticated && user">
          <DropdownMenuRoot>
            <DropdownMenuTrigger
              class="bottom-bar-user-trigger"
              :aria-label="
                profileBadgeCount > 0
                  ? `Menu compte (${profileBadgeCount})`
                  : 'Menu compte'
              "
            >
              <img
                :src="user.effective_avatar_url"
                :alt="user.effective_display_name"
                class="size-9 rounded-full border border-primary/30 object-cover"
              />
              <span
                v-if="profileBadgeCount > 0"
                class="topbar-cta-count"
              >
                {{ profileBadgeCount }}
              </span>
            </DropdownMenuTrigger>
            <DropdownMenuPortal>
              <DropdownMenuContent
                align="start"
                :side-offset="8"
                side="top"
                class="topbar-user-menu"
              >
                <DropdownMenuItem v-if="profileRoute" as-child>
                  <RouterLink :to="profileRoute" :class="menuItemClass">
                    <User class="size-4" />
                    Mon profil
                  </RouterLink>
                </DropdownMenuItem>
                <DropdownMenuItem v-if="inProgressRoute" as-child>
                  <RouterLink :to="inProgressRoute" :class="menuItemClass">
                    <Play class="size-4" />
                    {{ inProgressMenuLabel }}
                  </RouterLink>
                </DropdownMenuItem>
                <DropdownMenuItem v-if="isAdmin" as-child>
                  <RouterLink :to="reportsRoute" :class="menuItemClass">
                    <CircleAlert class="size-4" />
                    Signalements TTS
                    <span
                      v-if="ttsReportCount > 0"
                      class="ml-auto text-xs font-semibold text-primary"
                    >
                      {{ ttsReportCount }}
                    </span>
                  </RouterLink>
                </DropdownMenuItem>
                <DropdownMenuItem v-if="isEditMode" as-child>
                  <RouterLink :to="{ name: 'admin' }" :class="menuItemClass">
                    <Shield class="size-4" />
                    Administration
                  </RouterLink>
                </DropdownMenuItem>
                <DropdownMenuSeparator
                  v-if="hasPlayer || inProgressRoute || isAdmin || isEditMode"
                  class="topbar-user-menu-separator"
                />
                <DropdownMenuItem :class="menuItemClass" @select="handleLogout">
                  <LogOut class="size-4" />
                  Déconnexion
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenuPortal>
          </DropdownMenuRoot>
          <div
            v-if="isAdmin"
            class="flex items-center gap-0"
            role="group"
            aria-label="Mode d’édition"
          >
            <Button
              type="button"
              size="xs"
              variant="outline"
              :class="[
                'rounded-r-none',
                isEditMode
                  ? 'border-primary bg-primary! text-primary-foreground hover:bg-primary/90'
                  : 'border-border bg-black text-white hover:text-primary',
              ]"
              :aria-pressed="isEditMode"
              title="Mode édition"
              @click="setEditMode(true)"
            >
              <Pencil class="size-3.5" />
            </Button>
            <Button
              type="button"
              size="xs"
              variant="outline"
              :class="[
                'rounded-l-none border-l-0',
                !isEditMode
                  ? 'border-primary bg-primary! text-primary-foreground hover:bg-primary/90'
                  : 'border-border bg-black text-white hover:text-primary',
              ]"
              :aria-pressed="!isEditMode"
              title="Mode consultation"
              @click="setEditMode(false)"
            >
              <Eye class="size-3.5" />
            </Button>
          </div>
        </template>
        <Button v-else size="sm" @click="login">
          <LogIn class="size-4" />
          Connexion
        </Button>
      </div>

      <RouterLink
        v-if="!loading && isAuthenticated"
        :to="partieCta.to"
        class="bottom-bar-cta"
        :class="{ 'bottom-bar-cta-active': isPartieCtaActive }"
        :aria-label="partieCta.ariaLabel"
        :title="partieCta.ariaLabel"
      >
        <Play class="size-4" />
        <span>{{ partieCta.label }}</span>
        <span
          v-if="inProgressCount > 0"
          class="topbar-cta-count"
        >
          {{ inProgressCount }}
        </span>
      </RouterLink>
    </div>
  </nav>
</template>
