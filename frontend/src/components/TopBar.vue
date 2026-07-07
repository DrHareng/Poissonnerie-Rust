<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { LogIn, LogOut, Trophy, Shield, Swords, Medal } from '@lucide/vue'
import { useAuth } from '@/composables/useAuth'
import { Button } from '@/components/ui/button'

const route = useRoute()
const { user, player, isAuthenticated, hasPlayer, loading, login, logout } = useAuth()

const links = [
  { to: '/classement', label: 'Classement', icon: Trophy },
  { to: '/tournois', label: 'Tournois', icon: Medal },
  { to: '/sectorielles', label: 'Sectorielles', icon: Shield },
  { to: '/matchs', label: 'Matchs', icon: Swords },
]

const activePath = computed(() => route.path)
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
        <div v-else-if="isAuthenticated && user" class="flex items-center gap-3">
          <RouterLink
            v-if="hasPlayer && player"
            :to="{ name: 'joueur', params: { name: player.name } }"
            class="flex min-w-0 items-center gap-2 rounded-md transition-opacity hover:opacity-85"
          >
            <img
              :src="user.effective_avatar_url"
              :alt="user.effective_display_name"
              class="size-8 rounded-full border border-primary/30 object-cover"
            />
            <span class="truncate text-sm font-medium">{{ user.effective_display_name }}</span>
          </RouterLink>
          <div v-else class="flex min-w-0 items-center gap-2">
            <img
              :src="user.effective_avatar_url"
              :alt="user.effective_display_name"
              class="size-8 rounded-full border border-primary/30 object-cover"
            />
            <span class="truncate text-sm font-medium">{{ user.effective_display_name }}</span>
          </div>
          <Button variant="outline" size="sm" @click="logout">
            <LogOut class="size-4" />
            Déconnexion
          </Button>
        </div>
        <Button v-else size="sm" @click="login">
          <LogIn class="size-4" />
          Connexion Discord
        </Button>
      </div>
    </div>
  </header>
</template>
