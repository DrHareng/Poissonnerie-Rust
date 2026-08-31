<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import {
  ChevronDown,
  Eye,
  LogIn,
  LogOut,
  Map,
  Pencil,
  Play,
  Podium,
  Swords,
  Trophy,
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
import { withBase } from '@/lib/basePath'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

const route = useRoute()
const { user, player, isAuthenticated, hasPlayer, loading, login, logout } =
  useAuth()
const { isAdmin, isEditMode, setEditMode } = useAdminEditMode()
const { menuLabel: inProgressMenuLabel, menuRoute: inProgressRoute } =
  useMyInProgressMatches()

const links = [
  { to: '/scenarios', label: 'Scénarios', icon: Map },
  { to: '/matchs', label: 'Matchs', icon: Swords },
  { to: '/tournois', label: 'Tournois', icon: Trophy },
  { to: '/classement', label: 'Classement', icon: Podium },
]

const activePath = computed(() => route.path)

function isLinkActive(to: string) {
  const path = activePath.value
  if (to === '/classement') {
    return (
      path === '/classement' ||
      path.startsWith('/sectorielle') ||
      path.startsWith('/joueur')
    )
  }
  if (to === '/matchs') {
    return path.startsWith('/matchs')
  }
  if (to === '/tournois') {
    return path.startsWith('/tournoi')
  }
  if (to === '/scenarios') {
    return path.startsWith('/scenarios')
  }
  return path === to
}

const playerPageRoute = computed(() =>
  hasPlayer.value && player.value
    ? { name: 'joueur' as const, params: { name: player.value.name } }
    : null,
)

const profileRoute = computed(() => playerPageRoute.value)

const menuItemClass = cn(
  'relative flex w-full cursor-default select-none items-center gap-2 rounded-md px-2 py-1.5 text-sm outline-none',
  'transition-colors hover:bg-primary/10 hover:text-primary focus:bg-primary/10 focus:text-primary',
)

async function handleLogout() {
  await logout()
}
</script>

<template>
  <header class="topbar">
    <RouterLink to="/" class="topbar-brand">
      <img :src="withBase('/brand/favicon.png')" alt="" class="size-7 object-contain" />
      <div class="min-w-0">
        <p class="topbar-title">La Poissonnerie</p>
        <p class="topbar-subtitle">Communauté française Infinity sur TTS</p>
      </div>
    </RouterLink>

    <div class="flex flex-col gap-3 md:flex-row md:items-center md:gap-4">
      <nav class="topbar-nav" aria-label="Navigation principale">
        <RouterLink
          v-if="isAuthenticated"
          to="/partie"
          class="topbar-cta"
          :class="{ 'topbar-cta-active': activePath === '/partie' }"
        >
          <Play class="size-4" />
          Démarrer une partie
        </RouterLink>
        <RouterLink
          v-for="link in links"
          :key="link.to"
          :to="link.to"
          class="topbar-link"
          :class="{ 'topbar-link-active': isLinkActive(link.to) }"
        >
          <component :is="link.icon" class="size-4" />
          {{ link.label }}
        </RouterLink>
      </nav>

      <div class="topbar-auth">
        <div v-if="loading" class="text-sm text-muted-foreground">
          Connexion...
        </div>
        <template v-else-if="isAuthenticated && user">
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
              Édition
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
              Lecture
            </Button>
          </div>

          <DropdownMenuRoot>
            <DropdownMenuTrigger
              class="topbar-user-trigger"
              aria-label="Menu compte"
            >
              <img
                :src="user.effective_avatar_url"
                :alt="user.effective_display_name"
                class="size-8 rounded-full border border-primary/30 object-cover"
              />
              <span class="truncate text-sm font-medium">{{
                user.effective_display_name
              }}</span>
              <ChevronDown class="size-4 shrink-0 opacity-60" />
            </DropdownMenuTrigger>
            <DropdownMenuPortal>
              <DropdownMenuContent
                align="end"
                :side-offset="8"
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
                <DropdownMenuSeparator
                  v-if="hasPlayer || inProgressRoute"
                  class="topbar-user-menu-separator"
                />
                <DropdownMenuItem :class="menuItemClass" @select="handleLogout">
                  <LogOut class="size-4" />
                  Déconnexion
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenuPortal>
          </DropdownMenuRoot>
        </template>
        <Button v-else size="sm" @click="login">
          <LogIn class="size-4" />
          Connexion Discord
        </Button>
      </div>
    </div>
  </header>
</template>
