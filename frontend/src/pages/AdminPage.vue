<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Link2, Merge, Search, Trash2, Unlink, Users } from '@lucide/vue'
import { toast } from 'vue-sonner'
import {
  deleteUnusedPlayer,
  fetchAdminAccounts,
  linkPlayerAccount,
  mergePlayers,
} from '@/lib/api'
import type { AdminAccounts, AdminPlayerEntry, AdminUserEntry } from '@/types/elo'
import { useAuth } from '@/composables/useAuth'
import PlayerLink from '@/components/PlayerLink.vue'
import PlayerPicker from '@/components/PlayerPicker.vue'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'

const { isAdmin, loading: authLoading, initialized, refresh } = useAuth()

const accounts = ref<AdminAccounts | null>(null)
const loading = ref(true)
const linking = ref(false)
const merging = ref(false)
const unlinkingName = ref<string | null>(null)
const deletingName = ref<string | null>(null)
const selectedUserId = ref<string>()
const selectedPlayerName = ref<string>()
const keepPlayerName = ref<string>()
const aliasPlayerName = ref<string>()
const userQuery = ref('')
const playerQuery = ref('')
const unlinkedOnly = ref(false)

const users = computed(() => accounts.value?.users ?? [])
const players = computed(() => accounts.value?.players ?? [])

const selectedUser = computed(() =>
  users.value.find((user) => String(user.id) === selectedUserId.value),
)
const selectedPlayer = computed(() =>
  players.value.find((player) => player.name === selectedPlayerName.value),
)

const userOptions = computed(() =>
  [...users.value]
    .sort((left, right) => {
      const leftLinked = left.player_name ? 1 : 0
      const rightLinked = right.player_name ? 1 : 0
      return leftLinked - rightLinked || userLabel(left).localeCompare(userLabel(right), 'fr')
    })
    .map((user) => ({
      value: String(user.id),
      label: user.player_name
        ? `${userLabel(user)} — ${user.player_name}`
        : userLabel(user),
    })),
)

const playerOptions = computed(() =>
  [...players.value]
    .sort((left, right) => {
      const leftLinked = left.linked_user || left.discord_username ? 1 : 0
      const rightLinked = right.linked_user || right.discord_username ? 1 : 0
      return leftLinked - rightLinked || left.name.localeCompare(right.name, 'fr')
    })
    .map((player) => ({
      value: player.name,
      label: player.linked_user
        ? `${player.name} — ${player.linked_user.effective_display_name}`
        : player.discord_username
          ? `${player.name} — @${player.discord_username}`
          : player.name,
    })),
)

const filteredUsers = computed(() => {
  const needle = userQuery.value.trim().toLowerCase()
  return users.value.filter((user) => {
    if (unlinkedOnly.value && user.player_name) return false
    if (!needle) return true
    return (
      userLabel(user).toLowerCase().includes(needle)
      || user.username.toLowerCase().includes(needle)
      || (user.player_name?.toLowerCase().includes(needle) ?? false)
    )
  })
})

const filteredPlayers = computed(() => {
  const needle = playerQuery.value.trim().toLowerCase()
  return players.value.filter((player) => {
    if (unlinkedOnly.value && player.linked_user) return false
    if (!needle) return true
    return (
      player.name.toLowerCase().includes(needle)
      || (player.discord_username?.toLowerCase().includes(needle) ?? false)
      || (player.linked_user?.effective_display_name.toLowerCase().includes(needle) ?? false)
      || (player.linked_user?.username.toLowerCase().includes(needle) ?? false)
    )
  })
})

const unlinkedUserCount = computed(
  () => users.value.filter((user) => !user.player_name).length,
)
const unlinkedPlayerCount = computed(
  () => players.value.filter((player) => !player.linked_user).length,
)

const reassignmentWarning = computed(() => {
  const parts: string[] = []
  if (selectedUser.value?.player_name && selectedUser.value.player_name !== selectedPlayerName.value) {
    parts.push(
      `Le compte ${userLabel(selectedUser.value)} est déjà lié à ${selectedUser.value.player_name}.`,
    )
  }
  if (
    selectedPlayer.value?.linked_user
    && String(selectedPlayer.value.linked_user.id) !== selectedUserId.value
  ) {
    parts.push(
      `Le joueur ${selectedPlayer.value.name} est déjà lié à ${selectedPlayer.value.linked_user.effective_display_name}.`,
    )
  }
  return parts.join(' ')
})

const alreadyLinked = computed(
  () =>
    !!selectedUser.value?.player_name
    && selectedUser.value.player_name === selectedPlayerName.value,
)

const keepPlayer = computed(() =>
  players.value.find((player) => player.name === keepPlayerName.value),
)
const aliasPlayer = computed(() =>
  players.value.find((player) => player.name === aliasPlayerName.value),
)

const keepPlayerOptions = computed(() =>
  playerOptions.value.filter((option) => option.value !== aliasPlayerName.value),
)
const aliasPlayerOptions = computed(() =>
  playerOptions.value.filter((option) => option.value !== keepPlayerName.value),
)

const canMerge = computed(
  () =>
    !!keepPlayerName.value
    && !!aliasPlayerName.value
    && keepPlayerName.value !== aliasPlayerName.value,
)

function userLabel(user: AdminUserEntry) {
  return user.effective_display_name || user.display_name || user.username
}

function formatDate(unix: number) {
  return new Date(unix * 1000).toLocaleDateString('fr-FR', {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  })
}

function playerMatchCount(player: AdminPlayerEntry) {
  return player.wins + player.draws + player.losses
}

async function loadAccounts() {
  loading.value = true
  try {
    accounts.value = await fetchAdminAccounts()
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible de charger les comptes')
  } finally {
    loading.value = false
  }
}

async function applyAccounts(next: AdminAccounts, message: string) {
  accounts.value = next
  await refresh()
  toast.success(message)
}

async function linkSelected() {
  const user = selectedUser.value
  const playerName = selectedPlayerName.value
  if (!user || !playerName) {
    toast.error('Choisissez un utilisateur et un joueur.')
    return
  }

  if (
    reassignmentWarning.value
    && !window.confirm(`${reassignmentWarning.value}\n\nRemplacer le lien existant ?`)
  ) {
    return
  }

  linking.value = true
  try {
    const next = await linkPlayerAccount(playerName, user.id)
    selectedUserId.value = undefined
    selectedPlayerName.value = undefined
    await applyAccounts(next, `${userLabel(user)} lié à ${playerName}`)
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible de lier le compte')
  } finally {
    linking.value = false
  }
}

async function unlinkPlayer(player: AdminPlayerEntry) {
  const label = player.linked_user
    ? `${player.name} de ${player.linked_user.effective_display_name}`
    : player.name
  if (!window.confirm(`Délier ${label} ?`)) {
    return
  }

  unlinkingName.value = player.name
  try {
    const next = await linkPlayerAccount(player.name, null)
    await applyAccounts(next, `${player.name} n’est plus lié à un compte`)
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible de délier le joueur')
  } finally {
    unlinkingName.value = null
  }
}

async function unlinkUser(user: AdminUserEntry) {
  if (!user.player_name) return
  const player = players.value.find((entry) => entry.name === user.player_name)
  if (!player) {
    toast.error('Joueur lié introuvable')
    return
  }
  await unlinkPlayer(player)
}

function clearPlayerSelections(name: string) {
  if (selectedPlayerName.value === name) selectedPlayerName.value = undefined
  if (keepPlayerName.value === name) keepPlayerName.value = undefined
  if (aliasPlayerName.value === name) aliasPlayerName.value = undefined
}

async function deletePlayer(player: AdminPlayerEntry) {
  const extra = player.linked_user
    ? `\nLe compte ${player.linked_user.effective_display_name} ne sera plus lié.`
    : ''
  if (!window.confirm(`Supprimer le profil « ${player.name} » ?${extra}`)) {
    return
  }

  deletingName.value = player.name
  try {
    const next = await deleteUnusedPlayer(player.name)
    clearPlayerSelections(player.name)
    await applyAccounts(next, `${player.name} a été supprimé`)
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible de supprimer le joueur')
  } finally {
    deletingName.value = null
  }
}

async function mergeSelected() {
  const keep = keepPlayer.value
  const alias = aliasPlayer.value
  if (!keep || !alias) {
    toast.error('Choisissez le joueur à conserver et celui à fusionner.')
    return
  }

  if (
    !window.confirm(
      `Fusionner « ${alias.name} » dans « ${keep.name} » ?\n\nLes parties de ${alias.name} seront transférées, l’ELO de ${keep.name} sera recalculé, et le profil ${alias.name} sera supprimé.`,
    )
  ) {
    return
  }

  merging.value = true
  try {
    const next = await mergePlayers(keep.name, alias.name)
    clearPlayerSelections(alias.name)
    keepPlayerName.value = undefined
    aliasPlayerName.value = undefined
    await applyAccounts(next, `${alias.name} fusionné dans ${keep.name}`)
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible de fusionner les joueurs')
  } finally {
    merging.value = false
  }
}

watch(
  [initialized, isAdmin],
  () => {
    if (initialized.value && isAdmin.value && !accounts.value) {
      void loadAccounts()
    }
  },
  { immediate: true },
)
</script>

<template>
  <div class="page-stack">
    <nav class="page-title-tabs shrink-0" aria-label="Administration">
      <div class="page-title-tabs-list">
        <h1 class="page-title-tab page-title-tab--active" aria-current="page">
          Administration
        </h1>
      </div>
    </nav>

    <div v-if="authLoading || !initialized" class="text-sm text-muted-foreground">
      Chargement...
    </div>

    <Alert v-else-if="!isAdmin" variant="destructive" class="neon-panel-accent">
      <AlertTitle>Accès réservé</AlertTitle>
      <AlertDescription>
        Cette page n’est disponible que pour les administrateurs.
      </AlertDescription>
    </Alert>

    <template v-else>
      <div class="grid gap-4 xl:grid-cols-2">
      <Card class="neon-panel">
        <CardHeader>
          <CardTitle class="flex items-center gap-2">
            <Link2 class="size-5 text-primary" />
            Lier un utilisateur à un joueur
          </CardTitle>
          <CardDescription>
            Un compte Discord ne peut être lié qu’à un seul joueur. Si l’un des deux est déjà lié, le lien précédent est remplacé.
          </CardDescription>
        </CardHeader>
        <CardContent class="grid gap-4">
          <div class="grid gap-4 md:grid-cols-2">
            <div class="grid gap-2">
              <Label>Utilisateur</Label>
              <PlayerPicker
                v-model="selectedUserId"
                :options="userOptions"
                placeholder="Chercher un utilisateur"
                empty-message="Aucun utilisateur trouvé."
              />
            </div>
            <div class="grid gap-2">
              <Label>Joueur</Label>
              <PlayerPicker
                v-model="selectedPlayerName"
                :options="playerOptions"
                placeholder="Chercher un joueur"
                empty-message="Aucun joueur trouvé."
              />
            </div>
          </div>
          <p v-if="reassignmentWarning" class="text-sm text-muted-foreground">
            {{ reassignmentWarning }}
          </p>
          <Button
            class="w-fit"
            :disabled="linking || alreadyLinked || !selectedUserId || !selectedPlayerName"
            @click="linkSelected"
          >
            {{ linking ? 'Liaison...' : alreadyLinked ? 'Déjà liés' : 'Lier' }}
          </Button>
        </CardContent>
      </Card>

      <Card class="neon-panel">
        <CardHeader>
          <CardTitle class="flex items-center gap-2">
            <Merge class="size-5 text-primary" />
            Fusionner deux joueurs
          </CardTitle>
          <CardDescription>
            Les parties du profil fusionné sont transférées vers le profil conservé, puis l’ELO est recalculé.
          </CardDescription>
        </CardHeader>
        <CardContent class="grid gap-4">
          <div class="grid gap-4 md:grid-cols-2">
            <div class="grid gap-2">
              <Label>Conserver</Label>
              <PlayerPicker
                v-model="keepPlayerName"
                :options="keepPlayerOptions"
                placeholder="Joueur à garder"
                empty-message="Aucun joueur trouvé."
              />
            </div>
            <div class="grid gap-2">
              <Label>Fusionner dans le premier</Label>
              <PlayerPicker
                v-model="aliasPlayerName"
                :options="aliasPlayerOptions"
                placeholder="Profil à absorber"
                empty-message="Aucun joueur trouvé."
              />
            </div>
          </div>
          <Button
            class="w-fit"
            :disabled="merging || !canMerge"
            @click="mergeSelected"
          >
            {{ merging ? 'Fusion...' : 'Fusionner' }}
          </Button>
        </CardContent>
      </Card>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <Button
          type="button"
          size="xs"
          variant="outline"
          :class="unlinkedOnly ? 'border-primary bg-primary! text-primary-foreground hover:bg-primary/90' : ''"
          :aria-pressed="unlinkedOnly"
          @click="unlinkedOnly = !unlinkedOnly"
        >
          Non liés uniquement
        </Button>
        <span class="text-sm text-muted-foreground">
          {{ unlinkedUserCount }} compte{{ unlinkedUserCount > 1 ? 's' : '' }}
          et {{ unlinkedPlayerCount }} joueur{{ unlinkedPlayerCount > 1 ? 's' : '' }} sans lien
        </span>
      </div>

      <div class="grid gap-4 xl:grid-cols-2">
        <Card class="neon-panel page-panel-scroll">
          <CardHeader>
            <CardTitle class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
              <span class="flex items-center gap-2">
                <Users class="size-5 text-primary" />
                Utilisateurs
                <Badge variant="outline">{{ users.length }}</Badge>
              </span>
              <div class="relative w-full sm:w-56">
                <Search
                  class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
                />
                <Input
                  v-model="userQuery"
                  placeholder="rechercher un compte"
                  autocomplete="off"
                  class="pl-8"
                />
              </div>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div
              v-if="loading && !accounts"
              class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
            >
              Chargement des utilisateurs...
            </div>
            <div
              v-else-if="filteredUsers.length === 0"
              class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
            >
              Aucun utilisateur ne correspond.
            </div>
            <Table v-else>
              <TableHeader>
                <TableRow>
                  <TableHead>Compte</TableHead>
                  <TableHead>Joueur</TableHead>
                  <TableHead class="w-20" />
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow v-for="user in filteredUsers" :key="user.id">
                  <TableCell>
                    <div class="flex items-center gap-2">
                      <img
                        :src="user.effective_avatar_url"
                        :alt="userLabel(user)"
                        class="size-8 rounded-full border border-primary/30 object-cover"
                      />
                      <div class="min-w-0">
                        <p class="truncate font-medium">{{ userLabel(user) }}</p>
                        <p class="truncate text-xs text-muted-foreground">
                          @{{ user.username }}
                          <span v-if="user.is_admin"> · admin</span>
                          · {{ formatDate(user.last_login_at) }}
                        </p>
                      </div>
                    </div>
                  </TableCell>
                  <TableCell>
                    <PlayerLink
                      v-if="user.player_name"
                      :name="user.player_name"
                    />
                    <span v-else class="text-muted-foreground">Non lié</span>
                  </TableCell>
                  <TableCell class="text-right">
                    <Button
                      v-if="user.player_name"
                      type="button"
                      size="xs"
                      variant="ghost"
                      :disabled="unlinkingName === user.player_name"
                      @click="unlinkUser(user)"
                    >
                      <Unlink class="size-3.5" />
                      Délier
                    </Button>
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </CardContent>
        </Card>

        <Card class="neon-panel page-panel-scroll">
          <CardHeader>
            <CardTitle class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
              <span class="flex items-center gap-2">
                Joueurs
                <Badge variant="outline">{{ players.length }}</Badge>
              </span>
              <div class="relative w-full sm:w-56">
                <Search
                  class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
                />
                <Input
                  v-model="playerQuery"
                  placeholder="rechercher un joueur"
                  autocomplete="off"
                  class="pl-8"
                />
              </div>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div
              v-if="loading && !accounts"
              class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
            >
              Chargement des joueurs...
            </div>
            <div
              v-else-if="filteredPlayers.length === 0"
              class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
            >
              Aucun joueur ne correspond.
            </div>
            <Table v-else>
              <TableHeader>
                <TableRow>
                  <TableHead>Joueur</TableHead>
                  <TableHead>Compte</TableHead>
                  <TableHead class="w-20" />
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow v-for="player in filteredPlayers" :key="player.name">
                  <TableCell>
                    <div class="min-w-0">
                      <PlayerLink :name="player.name" />
                      <p class="text-xs text-muted-foreground">
                        {{ Math.round(player.rating) }} ELO
                        · {{ playerMatchCount(player) }} partie{{ playerMatchCount(player) > 1 ? 's' : '' }}
                      </p>
                    </div>
                  </TableCell>
                  <TableCell>
                    <template v-if="player.linked_user">
                      <p class="font-medium">{{ player.linked_user.effective_display_name }}</p>
                      <p class="text-xs text-muted-foreground">
                        @{{ player.linked_user.username }}
                      </p>
                    </template>
                    <template v-else-if="player.discord_username">
                      <p class="text-muted-foreground">@{{ player.discord_username }}</p>
                      <p class="text-xs text-muted-foreground">pas de compte</p>
                    </template>
                    <span v-else class="text-muted-foreground">Non lié</span>
                  </TableCell>
                  <TableCell class="text-right">
                    <div class="flex items-center justify-end gap-1">
                    <Button
                      v-if="player.linked_user || player.discord_username"
                      type="button"
                      size="xs"
                      variant="ghost"
                      :disabled="unlinkingName === player.name"
                      @click="unlinkPlayer(player)"
                    >
                      <Unlink class="size-3.5" />
                      Délier
                    </Button>
                    <Button
                      v-if="player.can_delete"
                      type="button"
                      size="xs"
                      variant="ghost"
                      class="text-destructive hover:text-destructive"
                      :disabled="deletingName === player.name"
                      title="Supprimer ce profil sans parties"
                      @click="deletePlayer(player)"
                    >
                      <Trash2 class="size-3.5" />
                      Supprimer
                    </Button>
                    </div>
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      </div>
    </template>
  </div>
</template>
