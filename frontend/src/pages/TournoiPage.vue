<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ArrowLeft, Check, X } from '@lucide/vue'
import { toast } from 'vue-sonner'
import {
  adminRegisterForTournament,
  closeTournamentRegistration,
  confirmTournamentMatch,
  fetchArmies,
  fetchRanking,
  fetchTournament,
  finalizePools,
  forfeitTournamentMatch,
  generatePoolMatches,
  openTournamentRegistration,
  registerForTournament,
  reviewRegistration,
  setupTournamentPools,
  startTournament,
  submitTournamentMatch,
} from '@/lib/api'
import type { Army, TournamentDetail, TournamentMatch, TournamentRegistration } from '@/types/elo'
import { useAuth } from '@/composables/useAuth'
import ArmyLogo from '@/components/ArmyLogo.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import SectorialPicker from '@/components/SectorialPicker.vue'
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
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

const props = defineProps<{ id: string }>()
const route = useRoute()
const router = useRouter()
const { isAdmin, hasPlayer, player, isAuthenticated } = useAuth()

const detail = ref<TournamentDetail | null>(null)
const allPlayers = ref<string[]>([])
const armies = ref<Army[]>([])
const loading = ref(true)
const registerArmyId = ref<string>()
const adminPlayerName = ref<string>()
const adminArmyId = ref<string>()
const registering = ref(false)
const adminAdding = ref(false)

const tournamentId = computed(() => Number(props.id || route.params.id))

const myRegistration = computed(() =>
  detail.value?.registrations.find(
    (r) => player.value && r.player_name.toLowerCase() === player.value.name.toLowerCase(),
  ),
)

const poolMatches = computed(() =>
  detail.value?.matches.filter((m) => m.phase === 'pool') ?? [],
)

const bracketMatches = computed(() =>
  detail.value?.matches.filter((m) => m.phase !== 'pool') ?? [],
)

const pendingRegistrations = computed(
  () => detail.value?.registrations.filter((r) => r.status === 'pending') ?? [],
)

const armiesRevealed = computed(
  () => detail.value?.status === 'started' || detail.value?.status === 'completed',
)

const activeRegistrations = computed(
  () =>
    detail.value?.registrations.filter((r) =>
      ['approved', 'waitlisted', 'pending'].includes(r.status),
    ) ?? [],
)

const availablePlayersForAdmin = computed(() => {
  if (!detail.value) return allPlayers.value
  const registered = new Set(
    detail.value.registrations.map((r) => r.player_name.toLowerCase()),
  )
  return allPlayers.value.filter((name) => !registered.has(name.toLowerCase()))
})

const canAdminAddPlayer = computed(
  () =>
    isAdmin.value &&
    detail.value &&
    (detail.value.status === 'registration_open' ||
      detail.value.status === 'registration_closed'),
)

const statusLabels: Record<string, string> = {
  draft: 'Brouillon',
  registration_open: 'Inscriptions ouvertes',
  registration_closed: 'Inscriptions fermées',
  started: 'En cours',
  completed: 'Terminé',
  pending: 'En attente',
  approved: 'Validé',
  waitlisted: 'Liste d\'attente',
  rejected: 'Refusé',
  scheduled: 'À jouer',
  submitted: 'En attente de confirmation',
  confirmed: 'Confirmé',
}

async function refresh() {
  loading.value = true
  try {
    const [tournament, ranking, armyList] = await Promise.all([
      fetchTournament(tournamentId.value),
      fetchRanking(),
      fetchArmies(),
    ])
    detail.value = tournament
    allPlayers.value = ranking.map((p) => p.name)
    armies.value = armyList
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Tournoi introuvable')
    router.push('/tournois')
  } finally {
    loading.value = false
  }
}

async function act(fn: () => Promise<unknown>, success: string) {
  try {
    await fn()
    toast.success(success)
    await refresh()
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur')
  }
}

async function register() {
  if (!registerArmyId.value) {
    toast.error('Choisissez une sectorielle.')
    return
  }
  registering.value = true
  try {
    await registerForTournament(tournamentId.value, Number(registerArmyId.value))
    toast.success('Inscription envoyée')
    registerArmyId.value = undefined
    await refresh()
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur')
  } finally {
    registering.value = false
  }
}

async function adminAddPlayer() {
  if (!adminPlayerName.value) {
    toast.error('Choisissez un joueur.')
    return
  }
  if (!adminArmyId.value) {
    toast.error('Choisissez une sectorielle.')
    return
  }
  adminAdding.value = true
  try {
    await adminRegisterForTournament(
      tournamentId.value,
      adminPlayerName.value,
      Number(adminArmyId.value),
    )
    toast.success(`${adminPlayerName.value} ajouté au tournoi`)
    adminPlayerName.value = undefined
    adminArmyId.value = undefined
    await refresh()
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur')
  } finally {
    adminAdding.value = false
  }
}

function showArmyForRegistration(reg: TournamentRegistration) {
  if (!reg.army_id) return false
  if (isAdmin.value || armiesRevealed.value) return true
  return (
    !!player.value &&
    reg.player_name.toLowerCase() === player.value.name.toLowerCase()
  )
}

async function review(reg: TournamentRegistration, action: 'approved' | 'rejected') {
  await act(
    () => reviewRegistration(tournamentId.value, reg.id, action),
    action === 'approved' ? 'Inscription validée' : 'Inscription refusée',
  )
}

async function autoSetupPools() {
  if (!detail.value) return
  const approved = detail.value.registrations
    .filter((r) => r.status === 'approved')
    .map((r) => r.player_name)

  const poolCount = detail.value.pool_count
  const perPool = Math.ceil(approved.length / poolCount)
  const poolNames = 'ABCDEFGH'.slice(0, poolCount).split('')

  const pools = poolNames.map((letter, index) => ({
    name: `Poule ${letter}`,
    position: index + 1,
    players: approved.slice(index * perPool, (index + 1) * perPool),
  }))

  await act(
    () => setupTournamentPools(tournamentId.value, pools),
    'Poules configurées',
  )
}

const matchForms = ref<Record<number, { p1: number; p2: number }>>({})

function getForm(match: TournamentMatch) {
  if (!matchForms.value[match.id]) {
    matchForms.value[match.id] = {
      p1: match.player1_objectives,
      p2: match.player2_objectives,
    }
  }
  return matchForms.value[match.id]
}

async function submitMatch(match: TournamentMatch) {
  const form = getForm(match)
  await act(
    () =>
      submitTournamentMatch(match.id, {
        player1_objectives: form.p1,
        player2_objectives: form.p2,
      }),
    'Résultat soumis',
  )
}

async function confirmMatch(match: TournamentMatch) {
  await act(() => confirmTournamentMatch(match.id), 'Résultat confirmé')
}

async function forfeitMatch(match: TournamentMatch, forfeitPlayer: string) {
  await act(
    () => forfeitTournamentMatch(match.id, forfeitPlayer),
    'Forfait enregistré',
  )
}

function canInteractWithMatch(match: TournamentMatch) {
  if (!isAuthenticated.value || !player.value) return false
  if (isAdmin.value) return true
  const name = player.value.name.toLowerCase()
  return (
    match.player1?.toLowerCase() === name || match.player2?.toLowerCase() === name
  )
}

function matchPlayerLabel(
  match: TournamentMatch,
  slot: 'player1' | 'player2',
) {
  const name = match[slot]
  const displayName =
    slot === 'player1' ? match.player1_display_name : match.player2_display_name
  return displayName || name || '?'
}

watch(() => tournamentId.value, refresh, { immediate: true })
onMounted(refresh)
</script>

<template>
  <div class="page-stack">
    <Button variant="ghost" class="w-fit" @click="router.push('/tournois')">
      <ArrowLeft class="size-4" />
      Retour aux tournois
    </Button>

    <div v-if="loading" class="text-muted-foreground">Chargement...</div>

    <template v-else-if="detail">
      <section class="page-header">
        <h1 class="page-title">{{ detail.name }}</h1>
        <p class="page-description">
          {{ statusLabels[detail.status] ?? detail.status }}
          — {{ detail.pool_count }} poules
        </p>
      </section>

      <!-- Inscriptions -->
      <Card
        v-if="detail.status === 'registration_open' || detail.status === 'registration_closed'"
        class="neon-panel"
      >
        <CardHeader>
          <CardTitle>Inscriptions</CardTitle>
          <CardDescription>
            Choisissez votre sectorielle à l'inscription. Elle sera révélée au démarrage du tournoi.
          </CardDescription>
        </CardHeader>
        <CardContent class="grid gap-4">
          <div
            v-if="hasPlayer && !myRegistration && detail.status === 'registration_open'"
            class="grid max-w-md gap-3"
          >
            <div class="grid gap-2">
              <Label>Sectorielle</Label>
              <SectorialPicker
                v-model="registerArmyId"
                :armies="armies"
                placeholder="Choisir une sectorielle"
              />
            </div>
            <Button :disabled="registering" @click="register">
              {{ registering ? 'Envoi...' : "S'inscrire" }}
            </Button>
          </div>
          <div v-else-if="myRegistration" class="flex flex-wrap items-center gap-3">
            <Badge variant="outline">
              {{ statusLabels[myRegistration.status] }}
              <span v-if="myRegistration.waitlist_position">
                (#{{ myRegistration.waitlist_position }})
              </span>
            </Badge>
            <ArmyLogo
              v-if="showArmyForRegistration(myRegistration)"
              :army-id="myRegistration.army_id!"
              :title="'Votre sectorielle'"
            />
            <span
              v-else-if="myRegistration.army_id || !armiesRevealed"
              class="text-sm text-muted-foreground"
            >
              Sectorielle secrète jusqu'au démarrage
            </span>
          </div>
          <p v-else-if="!isAuthenticated" class="text-sm text-muted-foreground">
            Connectez-vous pour vous inscrire.
          </p>
        </CardContent>
      </Card>

      <!-- Admin: ajout manuel -->
      <Card v-if="canAdminAddPlayer" class="neon-panel">
        <CardHeader>
          <CardTitle>Ajouter un joueur</CardTitle>
          <CardDescription>
            Inscription manuelle avec sectorielle (validée automatiquement).
          </CardDescription>
        </CardHeader>
        <CardContent class="grid max-w-md gap-3">
          <div class="grid gap-2">
            <Label>Joueur</Label>
            <Select v-model="adminPlayerName">
              <SelectTrigger>
                <SelectValue placeholder="Choisir un joueur" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="name in availablePlayersForAdmin"
                  :key="name"
                  :value="name"
                >
                  {{ name }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div class="grid gap-2">
            <Label>Sectorielle</Label>
            <SectorialPicker
              v-model="adminArmyId"
              :armies="armies"
              placeholder="Choisir une sectorielle"
            />
          </div>
          <Button :disabled="adminAdding" @click="adminAddPlayer">
            {{ adminAdding ? 'Ajout...' : 'Ajouter au tournoi' }}
          </Button>
        </CardContent>
      </Card>

      <!-- Admin controls -->
      <Card v-if="isAdmin" class="neon-panel-accent">
        <CardHeader>
          <CardTitle>Administration</CardTitle>
        </CardHeader>
        <CardContent class="flex flex-wrap gap-2">
          <Button
            v-if="detail.status === 'draft'"
            size="sm"
            @click="act(() => openTournamentRegistration(tournamentId), 'Inscriptions ouvertes')"
          >
            Ouvrir inscriptions
          </Button>
          <Button
            v-if="detail.status === 'registration_open'"
            size="sm"
            variant="outline"
            @click="act(() => closeTournamentRegistration(tournamentId), 'Inscriptions fermées')"
          >
            Fermer inscriptions
          </Button>
          <Button
            v-if="detail.status === 'registration_open' || detail.status === 'registration_closed'"
            size="sm"
            @click="act(() => startTournament(tournamentId), 'Tournoi démarré')"
          >
            Démarrer le tournoi
          </Button>
          <Button
            v-if="detail.status === 'started' && detail.pools.length === 0"
            size="sm"
            variant="outline"
            @click="autoSetupPools"
          >
            Répartir les poules (auto)
          </Button>
          <Button
            v-if="detail.status === 'started' && detail.pools.length > 0 && poolMatches.length === 0"
            size="sm"
            variant="outline"
            @click="act(() => generatePoolMatches(tournamentId), 'Matchs de poule générés')"
          >
            Générer matchs de poule
          </Button>
          <Button
            v-if="detail.status === 'started' && poolMatches.length > 0 && !detail.pools_finalized_at"
            size="sm"
            @click="act(() => finalizePools(tournamentId), 'Poules clôturées, arbre généré')"
          >
            Clôturer les poules
          </Button>
        </CardContent>
      </Card>

      <!-- Pending registrations (admin) -->
      <Card v-if="isAdmin && pendingRegistrations.length > 0" class="neon-panel">
        <CardHeader>
          <CardTitle>Inscriptions en attente</CardTitle>
        </CardHeader>
        <CardContent class="grid gap-2">
          <div
            v-for="reg in pendingRegistrations"
            :key="reg.id"
            class="flex items-center justify-between rounded border p-3"
          >
            <div class="flex items-center gap-2">
              <PlayerLink
                :name="reg.player_name"
                :display-name="reg.player_display_name"
              />
              <ArmyLogo v-if="reg.army_id" :army-id="reg.army_id" />
            </div>
            <div class="flex gap-2">
              <Button size="sm" @click="review(reg, 'approved')">
                <Check class="size-4" />
              </Button>
              <Button size="sm" variant="outline" @click="review(reg, 'rejected')">
                <X class="size-4" />
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      <!-- Liste des inscrits -->
      <Card
        v-if="activeRegistrations.length > 0 && (armiesRevealed || isAdmin)"
        class="neon-panel"
      >
        <CardHeader>
          <CardTitle>Inscrits</CardTitle>
          <CardDescription v-if="!armiesRevealed && isAdmin">
            Sectorielles visibles uniquement pour l'orga avant le démarrage.
          </CardDescription>
        </CardHeader>
        <CardContent class="grid gap-2 sm:grid-cols-2">
          <div
            v-for="reg in activeRegistrations"
            :key="reg.id"
            class="flex items-center justify-between rounded border px-3 py-2 text-sm"
          >
            <div class="flex items-center gap-2">
              <PlayerLink
                :name="reg.player_name"
                :display-name="reg.player_display_name"
              />
              <ArmyLogo
                v-if="showArmyForRegistration(reg)"
                :army-id="reg.army_id!"
              />
            </div>
            <Badge variant="outline" class="text-xs">
              {{ statusLabels[reg.status] }}
            </Badge>
          </div>
        </CardContent>
      </Card>

      <!-- Poules -->
      <Card v-if="detail.pools.length > 0" class="neon-panel">
        <CardHeader>
          <CardTitle>Poules</CardTitle>
        </CardHeader>
        <CardContent class="grid gap-4 md:grid-cols-2">
          <div
            v-for="pool in detail.pools"
            :key="pool.id"
            class="rounded-lg border p-4"
          >
            <h3 class="mb-2 font-semibold">{{ pool.name }}</h3>
            <ol class="space-y-1 text-sm">
              <li
                v-for="(pp, idx) in [...pool.players].sort((a, b) => b.points - a.points || b.objectives - a.objectives)"
                :key="pp.player_name"
                class="flex justify-between"
              >
                <span>{{ idx + 1 }}. <PlayerLink :name="pp.player_name" :display-name="pp.player_display_name" /></span>
                <span class="tabular-nums text-muted-foreground">{{ pp.points }} pts</span>
              </li>
            </ol>
          </div>
        </CardContent>
      </Card>

      <!-- Matchs poule -->
      <Card v-if="poolMatches.length > 0" class="neon-panel page-panel-scroll">
        <CardHeader>
          <CardTitle>Matchs de poule</CardTitle>
        </CardHeader>
        <CardContent class="grid gap-3">
          <div
            v-for="match in poolMatches"
            :key="match.id"
            class="rounded-lg border p-3"
          >
            <div class="mb-2 flex flex-wrap items-center justify-between gap-2">
              <span>
                <PlayerLink
                  :name="match.player1!"
                  :display-name="match.player1_display_name"
                />
                vs
                <PlayerLink
                  :name="match.player2!"
                  :display-name="match.player2_display_name"
                />
              </span>
              <Badge variant="outline">
                {{ match.is_forfeit ? 'Forfait' : statusLabels[match.status] }}
              </Badge>
            </div>

            <div v-if="match.status === 'confirmed'" class="text-sm tabular-nums">
              {{ match.player1_objectives }}-{{ match.player2_objectives }}
              ({{ match.player1_tournament_points }}-{{ match.player2_tournament_points }} pts)
            </div>

            <div
              v-else-if="canInteractWithMatch(match)"
              class="mt-2 flex flex-wrap items-end gap-2"
            >
              <div class="grid gap-1">
                <Label class="text-xs">Obj J1</Label>
                <Input
                  v-model.number="getForm(match).p1"
                  type="number"
                  min="0"
                  max="10"
                  class="w-20"
                />
              </div>
              <div class="grid gap-1">
                <Label class="text-xs">Obj J2</Label>
                <Input
                  v-model.number="getForm(match).p2"
                  type="number"
                  min="0"
                  max="10"
                  class="w-20"
                />
              </div>
              <Button size="sm" @click="submitMatch(match)">Soumettre</Button>
              <Button
                v-if="match.status === 'submitted'"
                size="sm"
                variant="outline"
                @click="confirmMatch(match)"
              >
                Confirmer
              </Button>
              <template v-if="isAdmin">
                <Button
                  size="sm"
                  variant="destructive"
                  @click="forfeitMatch(match, match.player1!)"
                >
                  FF {{ matchPlayerLabel(match, 'player1') }}
                </Button>
                <Button
                  size="sm"
                  variant="destructive"
                  @click="forfeitMatch(match, match.player2!)"
                >
                  FF {{ matchPlayerLabel(match, 'player2') }}
                </Button>
              </template>
            </div>
          </div>
        </CardContent>
      </Card>

      <!-- Arbre -->
      <Card v-if="bracketMatches.length > 0" class="neon-panel">
        <CardHeader>
          <CardTitle>Arbre</CardTitle>
        </CardHeader>
        <CardContent class="grid gap-3">
          <div
            v-for="match in bracketMatches"
            :key="match.id"
            class="rounded-lg border p-3"
          >
            <div class="mb-1 text-xs uppercase text-muted-foreground">{{ match.phase }}</div>
            <div class="flex flex-wrap items-center justify-between gap-2">
              <span>
                {{ matchPlayerLabel(match, 'player1') }}
                vs
                {{ matchPlayerLabel(match, 'player2') }}
              </span>
              <Badge variant="outline">{{ statusLabels[match.status] }}</Badge>
            </div>
            <div v-if="match.status === 'confirmed'" class="text-sm tabular-nums">
              {{ match.player1_objectives }}-{{ match.player2_objectives }}
            </div>
            <div
              v-else-if="match.player1 && match.player2 && canInteractWithMatch(match)"
              class="mt-2 flex flex-wrap items-end gap-2"
            >
              <Input v-model.number="getForm(match).p1" type="number" min="0" max="10" class="w-20" />
              <Input v-model.number="getForm(match).p2" type="number" min="0" max="10" class="w-20" />
              <Button size="sm" @click="submitMatch(match)">Soumettre</Button>
              <Button
                v-if="match.status === 'submitted'"
                size="sm"
                variant="outline"
                @click="confirmMatch(match)"
              >
                Confirmer
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>
    </template>
  </div>
</template>
