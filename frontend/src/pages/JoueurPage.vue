<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useTitle } from '@vueuse/core'
import { ArrowLeft } from '@lucide/vue'
import { toast } from 'vue-sonner'
import {
  fetchPlayer,
  fetchPlayerMatches,
  fetchPlayerTournaments,
  fetchRanking,
  updateProfile,
} from '@/lib/api'
import { pageTitle } from '@/lib/pageTitle'
import { useAuth } from '@/composables/useAuth'
import type { MatchOutcome, MatchRecord, PlayerProfile, PlayerTournamentResult, RankedPlayer } from '@/types/elo'
import MatchResultBadges from '@/components/MatchResultBadges.vue'
import WinDrawLossBar from '@/components/WinDrawLossBar.vue'
import ArmyLogo from '@/components/ArmyLogo.vue'
import PlayerLink from '@/components/PlayerLink.vue'
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

const route = useRoute()
const router = useRouter()
const { user, refresh: refreshAuth } = useAuth()

const player = ref<RankedPlayer | null>(null)
const profile = ref<PlayerProfile | null>(null)
const matches = ref<MatchRecord[]>([])
const tournamentResults = ref<PlayerTournamentResult[]>([])
const loading = ref(true)
const loadingMatches = ref(true)
const savingProfile = ref(false)

const localDisplayName = ref('')
const localAvatarUrl = ref('')

const playerName = computed(() => String(route.params.name ?? ''))

const title = useTitle()

const headerTitle = computed(() => profile.value?.display_name ?? player.value?.display_name ?? player.value?.name ?? '')

watch(
  () => headerTitle.value || playerName.value,
  (name) => {
    if (name) title.value = pageTitle(name)
  },
  { immediate: true },
)

function normalize(name: string) {
  return name.trim().toLowerCase()
}

function isSamePlayer(a: string, b: string) {
  return normalize(a) === normalize(b)
}

function flipOutcome(outcome: MatchOutcome): MatchOutcome {
  if (outcome === 'player1_win') return 'player2_win'
  if (outcome === 'player2_win') return 'player1_win'
  return 'draw'
}

function normalizeMatchForPlayer(match: MatchRecord, playerName: string): MatchRecord {
  if (isSamePlayer(match.player1, playerName)) {
    return match
  }

  return {
    ...match,
    player1: match.player2,
    player2: match.player1,
    player1_display_name: match.player2_display_name,
    player2_display_name: match.player1_display_name,
    player1_old: match.player2_old,
    player1_new: match.player2_new,
    player2_old: match.player1_old,
    player2_new: match.player1_new,
    player1_objectives: match.player2_objectives,
    player1_survivors: match.player2_survivors,
    player2_objectives: match.player1_objectives,
    player2_survivors: match.player1_survivors,
    player1_army_id: match.player2_army_id,
    player2_army_id: match.player1_army_id,
    outcome: flipOutcome(match.outcome),
  }
}

const matchRows = computed(() => {
  if (!player.value) return []

  return matches.value.map((match) => {
    const normalized = normalizeMatchForPlayer(match, player.value!.name)
    return {
      id: match.id,
      date: formatDate(match.recorded_at),
      normalized,
    }
  })
})

function formatDate(timestamp: number) {
  return new Intl.DateTimeFormat('fr-FR', {
    dateStyle: 'short',
    timeStyle: 'short',
  }).format(new Date(timestamp * 1000))
}

function syncProfileForm() {
  localDisplayName.value = user.value?.local_display_name ?? ''
  localAvatarUrl.value = user.value?.local_avatar_url ?? ''
}

async function loadPlayer() {
  const name = playerName.value
  if (!name) {
    player.value = null
    profile.value = null
    return
  }

  loading.value = true
  try {
    const [ranking, playerData] = await Promise.all([
      fetchRanking(),
      fetchPlayer(name),
    ])

    profile.value = playerData

    const ranked = ranking.find(
      (entry) => normalize(entry.name) === normalize(name),
    )

    player.value = ranked ?? {
      ...playerData,
      display_name: playerData.display_name,
      rank: ranking.length + 1,
      top_armies: [],
    }

    if (playerData.is_own_profile) {
      await refreshAuth()
      syncProfileForm()
    }
  } catch (error) {
    player.value = null
    profile.value = null
    toast.error(error instanceof Error ? error.message : 'Joueur introuvable')
    router.push('/classement')
  } finally {
    loading.value = false
  }
}

async function loadMatches() {
  const name = playerName.value
  if (!name) {
    matches.value = []
    return
  }

  loadingMatches.value = true
  try {
    matches.value = await fetchPlayerMatches(name)
  } catch {
    matches.value = []
  } finally {
    loadingMatches.value = false
  }
}

async function loadTournaments() {
  const name = playerName.value
  if (!name) {
    tournamentResults.value = []
    return
  }
  try {
    tournamentResults.value = await fetchPlayerTournaments(name)
  } catch {
    tournamentResults.value = []
  }
}

async function saveProfile() {
  savingProfile.value = true
  try {
    await updateProfile({
      local_display_name: localDisplayName.value,
      local_avatar_url: localAvatarUrl.value,
    })
    await refreshAuth()
    await loadPlayer()
    toast.success('Profil mis à jour.')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur inconnue')
  } finally {
    savingProfile.value = false
  }
}

async function resetDisplayName() {
  savingProfile.value = true
  try {
    await updateProfile({ clear_local_display_name: true })
    localDisplayName.value = ''
    await refreshAuth()
    await loadPlayer()
    toast.success('Pseudo Discord restauré.')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur inconnue')
  } finally {
    savingProfile.value = false
  }
}

async function resetAvatar() {
  savingProfile.value = true
  try {
    await updateProfile({ clear_local_avatar_url: true })
    localAvatarUrl.value = ''
    await refreshAuth()
    await loadPlayer()
    toast.success('Avatar Discord restauré.')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur inconnue')
  } finally {
    savingProfile.value = false
  }
}

async function refresh() {
  await Promise.all([loadPlayer(), loadMatches(), loadTournaments()])
}

watch(playerName, refresh, { immediate: true })
onMounted(refresh)
</script>

<template>
  <div class="page-stack">
    <Button variant="ghost" class="w-fit" @click="router.back()">
      <ArrowLeft class="size-4" />
      Retour
    </Button>

    <div
      v-if="loading"
      class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
    >
      Chargement du joueur...
    </div>

    <template v-else-if="player">
      <section class="page-header flex flex-col gap-4 sm:flex-row sm:items-center">
        <img
          v-if="profile?.avatar_url"
          :src="profile.avatar_url"
          :alt="headerTitle"
          class="size-20 rounded-full border border-primary/30 object-cover"
        />
        <div class="min-w-0">
          <h1 class="page-title flex flex-wrap items-baseline gap-x-2">
            <span>{{ headerTitle }}</span>
            <span
              v-if="profile?.discord_display_name"
              class="text-lg font-normal text-muted-foreground"
            >
              ({{ profile.discord_display_name }})
            </span>
          </h1>
          <p class="page-description">
            {{ player.name }} — Rang #{{ player.rank }} — {{ Math.round(player.rating) }} ELO
          </p>
        </div>
      </section>

      <Card v-if="profile?.is_own_profile" class="neon-panel">
        <CardHeader>
          <CardTitle>Mon profil</CardTitle>
          <CardDescription>
            Personnalisez le pseudo et l'avatar affichés à la place de ceux de Discord.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form class="flex flex-col gap-4" @submit.prevent="saveProfile">
            <div class="grid gap-2">
              <Label for="profile-display-name">Pseudo affiché</Label>
              <Input
                id="profile-display-name"
                v-model="localDisplayName"
                placeholder="Laisser vide pour utiliser le pseudo Discord"
                autocomplete="off"
              />
            </div>
            <div class="grid gap-2">
              <Label for="profile-avatar-url">URL de l'avatar</Label>
              <Input
                id="profile-avatar-url"
                v-model="localAvatarUrl"
                placeholder="Laisser vide pour utiliser l'avatar Discord"
                autocomplete="off"
                inputmode="url"
              />
            </div>
            <div class="flex flex-col gap-2 sm:flex-row sm:flex-wrap">
              <Button type="submit" :disabled="savingProfile">
                {{ savingProfile ? 'Enregistrement...' : 'Enregistrer' }}
              </Button>
              <Button
                type="button"
                variant="outline"
                :disabled="savingProfile"
                @click="resetDisplayName"
              >
                Restaurer le pseudo Discord
              </Button>
              <Button
                type="button"
                variant="outline"
                :disabled="savingProfile"
                @click="resetAvatar"
              >
                Restaurer l'avatar Discord
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>

      <WinDrawLossBar
        :wins="player.wins"
        :draws="player.draws"
        :losses="player.losses"
      />

      <Card v-if="tournamentResults.length > 0" class="neon-panel">
        <CardHeader>
          <CardTitle>Palmarès tournois</CardTitle>
        </CardHeader>
        <CardContent>
          <ul class="space-y-2">
            <li
              v-for="result in tournamentResults"
              :key="result.tournament_id"
              class="flex justify-between rounded border px-3 py-2 text-sm"
            >
              <span>{{ result.tournament_name }}</span>
              <span class="font-medium">{{ result.placement_label }}</span>
            </li>
          </ul>
        </CardContent>
      </Card>

      <Card class="neon-panel page-panel-scroll">
        <CardHeader class="lg:shrink-0">
          <CardTitle>Historique des parties</CardTitle>
          <CardDescription>
            Toutes les parties enregistrées pour ce joueur.
          </CardDescription>
        </CardHeader>
        <CardContent class="lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
          <div
            v-if="loadingMatches"
            class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
          >
            Chargement des parties...
          </div>

          <div
            v-else-if="matches.length === 0"
            class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
          >
            Aucune partie enregistrée pour ce joueur.
          </div>

          <Table v-else>
            <TableHeader class="sticky top-0 z-10 bg-card/95 backdrop-blur">
              <TableRow>
                <TableHead>Date</TableHead>
                <TableHead class="text-right">Joueur</TableHead>
                <TableHead class="w-10" aria-hidden="true" />
                <TableHead>Scénario</TableHead>
                <TableHead class="text-center">Résultat</TableHead>
                <TableHead class="w-10" aria-hidden="true" />
                <TableHead>Adversaire</TableHead>
                <TableHead class="text-right">ELO</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="row in matchRows" :key="row.id">
                <TableCell class="whitespace-nowrap text-muted-foreground">
                  {{ row.date }}
                </TableCell>
                <TableCell class="text-right font-medium">
                  {{ profile?.display_name ?? player.display_name }}
                </TableCell>
                <TableCell class="w-10 px-2">
                  <ArmyLogo :army-id="row.normalized.player1_army_id" />
                </TableCell>
                <TableCell class="text-sm text-muted-foreground">
                  {{ row.normalized.scenario_name ?? '—' }}
                </TableCell>
                <TableCell>
                  <MatchResultBadges :match="row.normalized" />
                </TableCell>
                <TableCell class="w-10 px-2">
                  <ArmyLogo :army-id="row.normalized.player2_army_id" />
                </TableCell>
                <TableCell>
                  <PlayerLink
                    :name="row.normalized.player2"
                    :display-name="row.normalized.player2_display_name"
                  />
                </TableCell>
                <TableCell class="text-right tabular-nums">
                  {{ Math.round(row.normalized.player1_old) }}
                  →
                  <span class="elo-score">{{ Math.round(row.normalized.player1_new) }}</span>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </template>
  </div>
</template>
