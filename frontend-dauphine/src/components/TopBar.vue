<script setup lang="ts">
import { ref } from 'vue'
import { useRoute } from 'vue-router'

type NavLink = {
  to: string
  label: string
}

const route = useRoute()

const links: NavLink[] = [
  { to: '/', label: 'Accueil' },
  { to: '/editions', label: 'Éditions' },
  { to: '/inscription', label: 'Inscriptions' },
  { to: '/live', label: 'Live' },
]

const menuOpen = ref(false)

function isActive(to: string) {
  if (to === '/') return route.path === '/'
  return route.path === to || route.path.startsWith(`${to}/`)
}

function closeMenu() {
  menuOpen.value = false
}
</script>

<template>
  <header class="dauphine-topbar">
    <div class="dauphine-topbar-inner">
      <RouterLink class="dauphine-brand" to="/" @click="closeMenu">
        Le Dauphiné
      </RouterLink>

      <nav class="dauphine-nav dauphine-nav--desktop" aria-label="Navigation principale">
        <RouterLink
          v-for="link in links"
          :key="link.to"
          :to="link.to"
          class="dauphine-nav-link"
          :class="{ 'dauphine-nav-link--active': isActive(link.to) }"
        >
          {{ link.label }}
        </RouterLink>
      </nav>

      <button
        type="button"
        class="dauphine-menu-toggle"
        :aria-expanded="menuOpen"
        aria-controls="dauphine-mobile-nav"
        @click="menuOpen = !menuOpen"
      >
        Menu
      </button>
    </div>

    <nav
      v-if="menuOpen"
      id="dauphine-mobile-nav"
      class="dauphine-nav dauphine-nav--mobile"
      aria-label="Navigation mobile"
    >
      <RouterLink
        v-for="link in links"
        :key="link.to"
        :to="link.to"
        class="dauphine-nav-link"
        :class="{ 'dauphine-nav-link--active': isActive(link.to) }"
        @click="closeMenu"
      >
        {{ link.label }}
      </RouterLink>
    </nav>
  </header>
</template>
