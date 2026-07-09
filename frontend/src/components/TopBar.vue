<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import {
  BarChart3,
  ChevronDown,
  LogIn,
  LogOut,
  Medal,
  Shield,
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
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

const route = useRoute()
const { user, player, isAuthenticated, hasPlayer, loading, login, logout } = useAuth()

const links = [
  { to: '/classement', label: 'Classement', icon: Trophy },
  { to: '/tournois', label: 'Tournois', icon: Medal },
  { to: '/sectorielles', label: 'Sectorielles', icon: Shield },
  { to: '/matchs', label: 'Matchs', icon: Swords },
]

const activePath = computed(() => route.path)

const playerPageRoute = computed(() =>
  hasPlayer.value && player.value
    ? { name: 'joueur' as const, params: { name: player.value.name } }
    : null,
)

const profileRoute = computed(() =>
  playerPageRoute.value ? { ...playerPageRoute.value, hash: '#profil' } : null,
)

const statsRoute = computed(() =>
  playerPageRoute.value ? { ...playerPageRoute.value, hash: '#stats' } : null,
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
  <header class="topbar">
    <RouterLink to="/classement" class="topbar-brand">
      <img src="/brand/favicon.png" alt="" class="size-7 object-contain" />
      <div class="min-w-0">
        <p class="topbar-title">La Poissonnerie</p>
        <p class="topbar-subtitle">Communauté française Infinity sur TTS</p>
      </div>
    </RouterLink>

    <div class="flex flex-col gap-3 md:flex-row md:items-center md:gap-4">
      <nav class="topbar-nav" aria-label="Navigation principale">
        <RouterLink
          v-for="link in links"
          :key="link.to"
          :to="link.to"
          class="topbar-link"
          :class="{ 'topbar-link-active': activePath === link.to || (link.to === '/tournois' && activePath.startsWith('/tournoi')) }"
        >
          <component :is="link.icon" class="size-4" />
          {{ link.label }}
        </RouterLink>
      </nav>

      <div class="topbar-auth">
        <div v-if="loading" class="text-sm text-muted-foreground">
          Connexion...
        </div>
        <DropdownMenuRoot v-else-if="isAuthenticated && user">
          <DropdownMenuTrigger
            class="topbar-user-trigger"
            aria-label="Menu compte"
          >
            <img
              :src="user.effective_avatar_url"
              :alt="user.effective_display_name"
              class="size-8 rounded-full border border-primary/30 object-cover"
            />
            <span class="truncate text-sm font-medium">{{ user.effective_display_name }}</span>
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
              <DropdownMenuItem v-if="statsRoute" as-child>
                <RouterLink :to="statsRoute" :class="menuItemClass">
                  <BarChart3 class="size-4" />
                  Mes stats
                </RouterLink>
              </DropdownMenuItem>
              <DropdownMenuSeparator v-if="hasPlayer" class="topbar-user-menu-separator" />
              <DropdownMenuItem :class="menuItemClass" @select="handleLogout">
                <LogOut class="size-4" />
                Déconnexion
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenuPortal>
        </DropdownMenuRoot>
        <Button v-else size="sm" @click="login">
          <LogIn class="size-4" />
          Connexion Discord
        </Button>
      </div>
    </div>
  </header>
</template>
