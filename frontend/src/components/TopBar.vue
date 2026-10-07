<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { useRoute, useRouter, type RouteLocationRaw } from 'vue-router'
import {
  ChevronDown,
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
import { withBase } from '@/lib/basePath'
import {
  isMainNavLinkActive,
  mainNavLinks,
  type MainNavLink,
} from '@/lib/mainNav'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

const route = useRoute()
const router = useRouter()
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

const links = mainNavLinks

const activePath = computed(() => route.path)
const openFlyout = ref<string | null>(null)
let flyoutCloseTimer: ReturnType<typeof setTimeout> | null = null

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

function isLinkActive(to: string) {
  return isMainNavLinkActive(activePath.value, to)
}

function isChildActive(to: RouteLocationRaw) {
  const resolved = router.resolve(to)
  if (resolved.path !== route.path) return false
  const resolvedTab = String(resolved.query.tab ?? '')
  const currentTab = String(route.query.tab ?? '')
  if (resolvedTab || currentTab) return resolvedTab === currentTab
  return true
}

function clearFlyoutTimer() {
  if (flyoutCloseTimer == null) return
  clearTimeout(flyoutCloseTimer)
  flyoutCloseTimer = null
}

function showFlyout(id: string) {
  clearFlyoutTimer()
  openFlyout.value = id
}

function onNavEnter(link: MainNavLink) {
  if (link.children?.length) showFlyout(link.to)
}

function onNavLeave(link: MainNavLink) {
  if (link.children?.length) scheduleHideFlyout()
}

function scheduleHideFlyout() {
  clearFlyoutTimer()
  flyoutCloseTimer = setTimeout(() => {
    openFlyout.value = null
    flyoutCloseTimer = null
  }, 160)
}

function onNavFocusOut(event: FocusEvent) {
  const current = event.currentTarget as HTMLElement
  const next = event.relatedTarget as Node | null
  if (next && current.contains(next)) return
  scheduleHideFlyout()
}

watch(
  () => route.fullPath,
  () => {
    clearFlyoutTimer()
    openFlyout.value = null
  },
)

onBeforeUnmount(clearFlyoutTimer)

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

    <div class="topbar-end">
      <nav class="topbar-nav" aria-label="Navigation principale" @keydown.escape="openFlyout = null">
        <div
          v-for="link in links"
          :key="link.to"
          class="topbar-nav-item"
          @pointerenter="onNavEnter(link)"
          @pointerleave="onNavLeave(link)"
          @focusin="onNavEnter(link)"
          @focusout="onNavFocusOut"
        >
          <RouterLink
            :to="link.to"
            class="topbar-link"
            :class="{ 'topbar-link-active': isLinkActive(link.to) }"
          >
            <component :is="link.icon" class="size-4" />
            {{ link.label }}
          </RouterLink>
          <div
            v-if="link.children && openFlyout === link.to"
            class="topbar-flyout"
          >
            <div class="topbar-flyout-panel">
              <RouterLink
                v-for="child in link.children"
                :key="child.label"
                :to="child.to"
                class="topbar-flyout-link"
                :class="{ 'topbar-flyout-link-active': isChildActive(child.to) }"
              >
                {{ child.label }}
              </RouterLink>
            </div>
          </div>
        </div>
      </nav>

      <RouterLink
        v-if="!loading && isAuthenticated"
        :to="partieCta.to"
        class="topbar-cta"
        :class="{ 'topbar-cta-active': isPartieCtaActive }"
        :aria-label="partieCta.ariaLabel"
        :title="partieCta.ariaLabel"
      >
        <Play class="size-4" />
        <span class="hidden md:inline">{{ partieCta.label }}</span>
        <span
          v-if="inProgressCount > 0"
          class="topbar-cta-count md:hidden"
        >
          {{ inProgressCount }}
        </span>
      </RouterLink>

      <div class="topbar-auth">
        <div v-if="loading" class="text-sm text-muted-foreground">
          Connexion...
        </div>
        <template v-else-if="isAuthenticated && user">
          <DropdownMenuRoot>
            <DropdownMenuTrigger
              class="topbar-user-trigger"
              :aria-label="
                profileBadgeCount > 0
                  ? `Menu compte (${profileBadgeCount})`
                  : 'Menu compte'
              "
            >
              <img
                :src="user.effective_avatar_url"
                :alt="user.effective_display_name"
                class="size-8 rounded-full border border-primary/30 object-cover"
              />
              <span class="hidden max-w-36 truncate text-sm font-medium md:inline">{{
                user.effective_display_name
              }}</span>
              <ChevronDown class="hidden size-4 shrink-0 opacity-60 md:block" />
              <span
                v-if="profileBadgeCount > 0"
                class="topbar-cta-count"
              >
                {{ profileBadgeCount }}
              </span>
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
          <span class="hidden sm:inline">Connexion Discord</span>
          <span class="sm:hidden">Connexion</span>
        </Button>
      </div>
    </div>
  </header>
</template>
