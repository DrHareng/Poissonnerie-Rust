<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ArrowLeft, Check, X } from '@lucide/vue'
import { toast } from 'vue-sonner'
import {
  adminRegisterForTournament,
  closeTournamentRegistration,
  confirmTournamentMatch,
  correctTournamentMatch,
  fetchArmies,
  fetchRanking,
  fetchTournament,
  finalizePools,
  forfeitTournamentMatch,
  unplayedTournamentMatch,
  generatePoolMatches,
  openTournamentRegistration,
  registerForTournament,
  reviewRegistration,
  setupTournamentBracket,
  setupTournamentPools,
  startTournament,
  submitTournamentMatch,
} from '@/lib/api'
import type {
  Army,
  Pool,
  PoolPlayer,
  RankedPlayer,
  TournamentDetail,
  TournamentMatch,
  TournamentRegistration,
} from '@/types/elo'
import { useAuth } from '@/composables/useAuth'
import ArmyLogo from '@/components/ArmyLogo.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import PlayerPicker from '@/components/PlayerPicker.vue'
import SectorialPicker from '@/components/SectorialPicker.vue'
import TournamentMatchCard from '@/components/TournamentMatchCard.vue'
import type { TournamentMatchForm } from '@/components/TournamentMatchCard.vue'
import { formatRegistrationSummary, topFourDisplayRows } from '@/lib/tournamentDisplay'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Label } from '@/components/ui/label'

const props = defineProps<{ id: string }>()
const route = useRoute()
const router = useRouter()
const { isAdmin, hasPlayer, player, isAuthenticated } = useAuth()

const detail = ref<TournamentDetail | null>(null)
const rankedPlayers = ref<RankedPlayer[]>([])
const armies = ref<Army[]>([])
const loading = ref(true)
const registerArmyId = ref<string>()
const adminPlayerName = ref<string>()
const adminArmyId = ref<string>()
const registering = ref(false)
const adminAdding = ref(false)
const savingPools = ref(false)
const savingBracket = ref(false)

interface ManualPoolSetup {
  name: string
  position: number
  players: string[]
}

interface ManualBracketSlot {
  bracket_slot: number
  player1?: string
  player2?: string
  quarter_player1?: string
}

const manualPools = ref<ManualPoolSetup[]>([])
const poolPickerValues = ref<(string | undefined)[]>([])
const manualBracket = ref<ManualBracketSlot[]>([])

const tournamentId = computed(() => Number(props.id || route.params.id))

const myRegistration = computed(() =>
  detail.value?.registrations.find(
    (r) => player.value && r.player_name.toLowerCase() === player.value.name.toLowerCase(),
  ),
)

const poolMatches = computed(() =>
  detail.value?.matches.filter((m) => m.phase === 'pool') ?? [],
)

const selectedPoolId = ref<number | null>(null)

const myPoolMatches = computed(() => {
  if (!player.value) return []
  const name = player.value.name.toLowerCase()
  return poolMatches.value.filter(
    (match) =>
      match.player1?.toLowerCase() === name
      || match.player2?.toLowerCase() === name,
  )
})

function poolMatchesForPool(poolId: number) {
  return poolMatches.value.filter((match) => match.pool_id === poolId)
}

function togglePoolDetail(poolId: number) {
  selectedPoolId.value = selectedPoolId.value === poolId ? null : poolId
}

function isPoolSelected(poolId: number) {
  return selectedPoolId.value === poolId
}

function selectedPool() {
  return sortedPools.value.find((pool) => pool.id === selectedPoolId.value) ?? null
}

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
  if (!detail.value) {
    return rankedPlayers.value.map((player) => ({
      value: player.name,
      label: player.display_name,
    }))
  }
  const registered = new Set(
    detail.value.registrations.map((r) => r.player_name.toLowerCase()),
  )
  return rankedPlayers.value
    .filter((player) => !registered.has(player.name.toLowerCase()))
    .map((player) => ({
      value: player.name,
      label: player.display_name,
    }))
})

const canAdminAddPlayer = computed(
  () =>
    isAdmin.value &&
    detail.value &&
    (detail.value.status === 'registration_open' ||
      detail.value.status === 'registration_closed'),
)

const canEditPools = computed(
  () =>
    isAdmin.value &&
    detail.value?.status === 'started' &&
    poolMatches.value.length === 0,
)

const canEditBracket = computed(
  () =>
    isAdmin.value &&
    detail.value?.status === 'started' &&
    !!detail.value?.pools_finalized_at &&
    bracketMatches.value.every((match) => match.status !== 'confirmed'),
)

const sortedPools = computed(() =>
  [...(detail.value?.pools ?? [])].sort((a, b) => a.position - b.position),
)

const isRoundOf16Format = computed(
  () => detail.value?.bracket_format === 'round_of_16',
)

const isQuartersDirectFormat = computed(
  () => detail.value?.bracket_format === 'quarters_direct',
)

const approvedRegistrations = computed(
  () => detail.value?.registrations.filter((r) => r.status === 'approved') ?? [],
)

const registrationByPlayer = computed(() => {
  const map = new Map<string, TournamentRegistration>()
  for (const registration of detail.value?.registrations ?? []) {
    map.set(registration.player_name.toLowerCase(), registration)
  }
  return map
})

const assignedPlayerNames = computed(
  () =>
    new Set(
      manualPools.value.flatMap((pool) =>
        pool.players.map((name) => name.toLowerCase()),
      ),
    ),
)

const unassignedPlayerOptions = computed(() =>
  approvedRegistrations.value
    .filter(
      (registration) =>
        !assignedPlayerNames.value.has(registration.player_name.toLowerCase()),
    )
    .map((registration) => ({
      value: registration.player_name,
      label: registration.player_display_name ?? registration.player_name,
    })),
)

const statusLabels: Record<string, string> = {
  pending: 'En attente',
  approved: 'Validé',
  waitlisted: 'Liste d\'attente',
  rejected: 'Refusé',
  scheduled: 'À jouer',
  submitted: 'En attente de confirmation',
  confirmed: 'Confirmé',
}

const phaseLabels: Record<string, string> = {
  pool: 'Poule',
  round_of_16: 'Huitièmes de finale',
  quarter: 'Quart de finale',
  semi: 'Demi-finale',
  final: 'Finale',
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
    rankedPlayers.value = ranking
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
    toast.success(
      `${availablePlayersForAdmin.value.find((p) => p.value === adminPlayerName.value)?.label ?? adminPlayerName.value} ajouté au tournoi`,
    )
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

function initManualPools() {
  if (!detail.value) {
    manualPools.value = []
    poolPickerValues.value = []
    return
  }

  const letters = 'ABCDEFGH'.slice(0, detail.value.pool_count).split('')

  if (detail.value.pools.length > 0) {
    manualPools.value = detail.value.pools.map((pool) => ({
      name: pool.name,
      position: pool.position,
      players: pool.players.map((player) => player.player_name),
    }))
  } else {
    manualPools.value = letters.map((letter, index) => ({
      name: `Poule ${letter}`,
      position: index + 1,
      players: [],
    }))
  }

  poolPickerValues.value = manualPools.value.map(() => undefined)
}

function registrationForPlayer(playerName: string) {
  return registrationByPlayer.value.get(playerName.toLowerCase())
}

function armyIdForPlayer(playerName: string | null | undefined) {
  if (!playerName) return undefined
  const registration = registrationForPlayer(playerName)
  if (!registration?.army_id || !showArmyForRegistration(registration)) return undefined
  return registration.army_id
}

function matchPlayerArmyId(match: TournamentMatch, slot: 'player1' | 'player2') {
  const fromMatch = slot === 'player1' ? match.player1_army_id : match.player2_army_id
  if (fromMatch) return fromMatch
  return armyIdForPlayer(match[slot])
}

function poolPlayerArmyId(pp: PoolPlayer) {
  return pp.army_id ?? armyIdForPlayer(pp.player_name)
}

function addPlayerToPool(poolIndex: number) {
  const playerName = poolPickerValues.value[poolIndex]
  if (!playerName) {
    toast.error('Choisissez un joueur.')
    return
  }

  const pool = manualPools.value[poolIndex]
  if (!pool) return

  if (pool.players.length >= 6) {
    toast.error('Maximum 6 joueurs par poule.')
    return
  }

  if (pool.players.some((name) => name.toLowerCase() === playerName.toLowerCase())) {
    return
  }

  pool.players.push(playerName)
  poolPickerValues.value[poolIndex] = undefined
}

function removePlayerFromPool(poolIndex: number, playerName: string) {
  const pool = manualPools.value[poolIndex]
  if (!pool) return
  pool.players = pool.players.filter((name) => name !== playerName)
}

async function saveManualPools() {
  if (!detail.value) return

  savingPools.value = true
  try {
    await setupTournamentPools(
      tournamentId.value,
      manualPools.value.map((pool) => ({
        name: pool.name,
        position: pool.position,
        players: [...pool.players],
      })),
    )
    toast.success('Poules enregistrées.')
    await refresh()
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur')
  } finally {
    savingPools.value = false
  }
}

function sortedPoolPlayers(pool: Pool) {
  return [...pool.players].sort(
    (a, b) => b.points - a.points || b.objectives - a.objectives,
  )
}

function poolLetter(pool: Pool, index: number) {
  const match = pool.name.match(/Poule\s+([A-H])/i)
  return match?.[1]?.toUpperCase() ?? String.fromCharCode(65 + index)
}

function playerOptionLabel(poolIndex: number, rank: number, player: PoolPlayer) {
  const ordinal = rank === 1 ? '1er' : rank === 2 ? '2ème' : `${rank}ème`
  const letter = poolLetter(sortedPools.value[poolIndex], poolIndex)
  const name = player.player_display_name ?? player.player_name
  return `${ordinal} ${letter} — ${name}`
}

const qualifiedPlayerOptions = computed(() => {
  if (!detail.value) return []
  const topN = detail.value.bracket_format === 'round_of_16' ? 3 : 2
  const options: { value: string; label: string }[] = []
  sortedPools.value.forEach((pool, poolIndex) => {
    sortedPoolPlayers(pool).slice(0, topN).forEach((pp, rankIndex) => {
      options.push({
        value: pp.player_name,
        label: playerOptionLabel(poolIndex, rankIndex + 1, pp),
      })
    })
  })
  return options
})

function defaultBracketSlots(): ManualBracketSlot[] {
  if (!detail.value) return []
  const ranked = sortedPools.value.map(sortedPoolPlayers)
  const first = (index: number) => ranked[index]?.[0]?.player_name
  const second = (index: number) => ranked[index]?.[1]?.player_name
  const third = (index: number) => ranked[index]?.[2]?.player_name

  if (detail.value.bracket_format === 'round_of_16') {
    return [
      { bracket_slot: 0, player1: second(3), player2: third(1), quarter_player1: first(0) },
      { bracket_slot: 1, player1: second(2), player2: third(0), quarter_player1: first(1) },
      { bracket_slot: 2, player1: second(0), player2: third(3), quarter_player1: first(2) },
      { bracket_slot: 3, player1: second(1), player2: third(2), quarter_player1: first(3) },
    ]
  }

  if (detail.value.bracket_format === 'quarters_direct') {
    return [
      { bracket_slot: 0, player1: second(3), player2: first(0) },
      { bracket_slot: 1, player1: second(2), player2: first(1) },
      { bracket_slot: 2, player1: second(0), player2: first(2) },
      { bracket_slot: 3, player1: second(1), player2: first(3) },
    ]
  }

  return Array.from({ length: 8 }, (_, index) => ({ bracket_slot: index }))
}

function initManualBracket() {
  if (!detail.value?.pools_finalized_at) {
    manualBracket.value = []
    return
  }

  const format = detail.value.bracket_format
  const firstPhase = format === 'quarters_direct' ? 'quarter' : 'round_of_16'
  const existing = detail.value.matches.filter(
    (match) => match.phase === firstPhase && match.player1 && match.player2,
  )

  if (existing.length > 0) {
    manualBracket.value = [...existing]
      .sort((a, b) => (a.bracket_slot ?? 0) - (b.bracket_slot ?? 0))
      .map((match) => {
        const slot = match.bracket_slot ?? 0
        let quarter_player1: string | undefined
        if (format === 'round_of_16') {
          const quarterMatch = detail.value!.matches.find(
            (candidate) => candidate.phase === 'quarter' && candidate.bracket_slot === slot,
          )
          quarter_player1 = quarterMatch?.player1 ?? undefined
        }
        return {
          bracket_slot: slot,
          player1: match.player1!,
          player2: match.player2!,
          quarter_player1,
        }
      })
    return
  }

  manualBracket.value = defaultBracketSlots()
}

function bracketSlotTitle(slot: number) {
  if (isQuartersDirectFormat.value) {
    return `Quart de finale ${slot + 1}`
  }
  if (isRoundOf16Format.value) {
    return `Barrage ${slot + 1}`
  }
  return `Huitième ${slot + 1}`
}

async function saveManualBracket() {
  for (const slot of manualBracket.value) {
    if (!slot.player1 || !slot.player2) {
      toast.error('Renseignez tous les joueurs de l\'arbre.')
      return
    }
    if (isRoundOf16Format.value && !slot.quarter_player1) {
      toast.error('Renseignez le 1er de poule en attente pour chaque barrage.')
      return
    }
  }

  savingBracket.value = true
  try {
    await setupTournamentBracket(
      tournamentId.value,
      manualBracket.value.map((slot) => ({
        bracket_slot: slot.bracket_slot,
        player1: slot.player1!,
        player2: slot.player2!,
        quarter_player1: slot.quarter_player1,
      })),
    )
    toast.success('Arbre enregistré.')
    await refresh()
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur')
  } finally {
    savingBracket.value = false
  }
}

const matchForms = ref<Record<number, { p1: number; p2: number; s1: number; s2: number }>>({})

function getForm(match: TournamentMatch) {
  if (!matchForms.value[match.id]) {
    matchForms.value[match.id] = {
      p1: match.player1_objectives,
      p2: match.player2_objectives,
      s1: match.player1_survivors,
      s2: match.player2_survivors,
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
        player1_survivors: form.s1,
        player2_survivors: form.s2,
      }),
    'Résultat soumis',
  )
}

async function confirmMatch(match: TournamentMatch) {
  await act(() => confirmTournamentMatch(match.id), 'Résultat confirmé')
}

async function correctMatch(match: TournamentMatch, form: TournamentMatchForm) {
  if (
    Number.isNaN(form.p1) ||
    Number.isNaN(form.p2) ||
    form.p1 < 0 ||
    form.p2 < 0 ||
    form.p1 > 10 ||
    form.p2 > 10
  ) {
    toast.error('Scores invalides.')
    return
  }

  await act(
    () =>
      correctTournamentMatch(match.id, {
        player1_objectives: form.p1,
        player2_objectives: form.p2,
        player1_survivors: form.s1,
        player2_survivors: form.s2,
      }),
    'Score corrigé',
  )
  delete matchForms.value[match.id]
}

async function forfeitMatch(match: TournamentMatch, forfeitPlayer: string) {
  await act(
    () => forfeitTournamentMatch(match.id, forfeitPlayer),
    'Forfait enregistré',
  )
}

async function markMatchUnplayed(match: TournamentMatch) {
  await act(
    () => unplayedTournamentMatch(match.id),
    'Match non joué enregistré',
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

function matchStatusLabel(match: TournamentMatch) {
  if (match.is_unplayed) return 'Non joué'
  return match.is_forfeit ? 'Forfait' : statusLabels[match.status] ?? match.status
}

watch(() => tournamentId.value, refresh, { immediate: true })
watch(detail, () => {
  initManualPools()
  initManualBracket()
  if (
    selectedPoolId.value
    && !detail.value?.pools.some((pool) => pool.id === selectedPoolId.value)
  ) {
    selectedPoolId.value = null
  }
}, { deep: true })
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
          {{ detail.display_status }}
          — {{ formatRegistrationSummary(detail.approved_count, detail.waitlist_count) }}
        </p>
      </section>

      <!-- Podium -->
      <Card
        v-if="detail.status === 'completed' && detail.top_four?.length"
        class="neon-panel"
      >
        <CardHeader>
          <CardTitle>Top 4</CardTitle>
        </CardHeader>
        <CardContent>
          <ol class="space-y-2">
            <li
              v-for="row in topFourDisplayRows(detail.top_four ?? [])"
              :key="row.label"
              class="flex items-center gap-3 rounded border px-3 py-2"
            >
              <span class="w-10 shrink-0 font-display text-sm font-semibold text-primary">
                {{ row.label }}
              </span>
              <div class="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">
                <template
                  v-for="(entry, index) in row.entries"
                  :key="entry.player_name"
                >
                  <span
                    v-if="index > 0"
                    class="text-muted-foreground"
                  >
                    ·
                  </span>
                  <PlayerLink
                    :name="entry.player_name"
                    :display-name="entry.player_display_name"
                    class="font-medium"
                  />
                  <ArmyLogo
                    v-if="armyIdForPlayer(entry.player_name)"
                    :army-id="armyIdForPlayer(entry.player_name)!"
                    class="shrink-0"
                  />
                </template>
              </div>
            </li>
          </ol>
        </CardContent>
      </Card>

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
            class="flex flex-wrap items-end gap-3"
          >
            <div class="grid min-w-[14rem] flex-1 gap-2">
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
        <CardContent class="flex flex-wrap items-end gap-3">
          <div class="grid min-w-[14rem] flex-1 gap-2">
            <Label>Joueur</Label>
            <PlayerPicker
              v-model="adminPlayerName"
              :options="availablePlayersForAdmin"
              placeholder="Tapez pour chercher un joueur"
            />
          </div>
          <div class="grid min-w-[14rem] flex-1 gap-2">
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

      <!-- Configuration manuelle des poules -->
      <Card v-if="canEditPools" class="neon-panel">
        <CardHeader>
          <CardTitle>Configurer les poules</CardTitle>
          <CardDescription>
            Répartissez manuellement les joueurs validés dans chaque poule (6 max.).
          </CardDescription>
        </CardHeader>
        <CardContent class="grid gap-4 md:grid-cols-2">
          <div
            v-for="(pool, poolIndex) in manualPools"
            :key="pool.position"
            class="rounded-lg border p-4"
          >
            <div class="mb-3 flex items-center justify-between gap-2">
              <h3 class="font-semibold">{{ pool.name }}</h3>
              <span class="text-xs text-muted-foreground">{{ pool.players.length }}/6</span>
            </div>

            <ul class="mb-3 space-y-2">
              <li
                v-for="playerName in pool.players"
                :key="playerName"
                class="flex items-center justify-between gap-2 rounded border px-2 py-1.5 text-sm"
              >
                <div class="flex min-w-0 items-center gap-2">
                  <PlayerLink
                    :name="playerName"
                    :display-name="registrationForPlayer(playerName)?.player_display_name"
                  />
                  <ArmyLogo
                    v-if="registrationForPlayer(playerName)?.army_id"
                    :army-id="registrationForPlayer(playerName)!.army_id!"
                  />
                </div>
                <Button
                  size="sm"
                  variant="ghost"
                  @click="removePlayerFromPool(poolIndex, playerName)"
                >
                  <X class="size-4" />
                </Button>
              </li>
              <li v-if="pool.players.length === 0" class="text-sm text-muted-foreground">
                Aucun joueur
              </li>
            </ul>

            <div
              v-if="pool.players.length < 6 && unassignedPlayerOptions.length > 0"
              class="flex items-end gap-2"
            >
              <div class="grid min-w-0 flex-1 gap-1">
                <Label class="text-xs">Ajouter un joueur</Label>
                <PlayerPicker
                  v-model="poolPickerValues[poolIndex]"
                  :options="unassignedPlayerOptions"
                  placeholder="Chercher un joueur"
                />
              </div>
              <Button size="sm" @click="addPlayerToPool(poolIndex)">
                Ajouter
              </Button>
            </div>
          </div>

          <div class="flex flex-wrap items-center justify-between gap-3 border-t pt-4 md:col-span-2">
            <p class="text-sm text-muted-foreground">
              {{ assignedPlayerNames.size }}/{{ approvedRegistrations.length }} joueurs répartis
              <span v-if="unassignedPlayerOptions.length > 0">
                — {{ unassignedPlayerOptions.length }} non assigné(s)
              </span>
            </p>
            <Button :disabled="savingPools" @click="saveManualPools">
              {{ savingPools ? 'Enregistrement...' : 'Enregistrer les poules' }}
            </Button>
          </div>
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
            @click="act(() => finalizePools(tournamentId), 'Poules clôturées')"
          >
            Clôturer les poules
          </Button>
        </CardContent>
      </Card>

      <!-- Configuration manuelle de l'arbre -->
      <Card v-if="canEditBracket" class="neon-panel">
        <CardHeader>
          <CardTitle>Configurer l'arbre</CardTitle>
          <CardDescription>
            Définissez manuellement les affrontements du premier tour de l'arbre.
          </CardDescription>
        </CardHeader>
        <CardContent class="grid gap-4 md:grid-cols-2">
          <div
            v-for="slot in manualBracket"
            :key="slot.bracket_slot"
            class="rounded-lg border p-4"
          >
            <h3 class="mb-3 font-semibold">{{ bracketSlotTitle(slot.bracket_slot) }}</h3>
            <div class="grid gap-3">
              <div class="grid gap-1">
                <Label>Joueur 1</Label>
                <PlayerPicker
                  v-model="slot.player1"
                  :options="qualifiedPlayerOptions"
                  placeholder="Choisir un joueur"
                />
              </div>
              <div class="grid gap-1">
                <Label>Joueur 2</Label>
                <PlayerPicker
                  v-model="slot.player2"
                  :options="qualifiedPlayerOptions"
                  placeholder="Choisir un joueur"
                />
              </div>
              <div v-if="isRoundOf16Format" class="grid gap-1">
                <Label>1er de poule en attente (quart de finale)</Label>
                <PlayerPicker
                  v-model="slot.quarter_player1"
                  :options="qualifiedPlayerOptions"
                  placeholder="Choisir le 1er de poule"
                />
              </div>
            </div>
          </div>

          <div class="flex flex-wrap items-center justify-between gap-3 border-t pt-4 md:col-span-2">
            <Button
              size="sm"
              variant="outline"
              @click="manualBracket = defaultBracketSlots()"
            >
              Réinitialiser les appariements
            </Button>
            <Button :disabled="savingBracket" @click="saveManualBracket">
              {{ savingBracket ? 'Enregistrement...' : 'Enregistrer l\'arbre' }}
            </Button>
          </div>
        </CardContent>
      </Card>

      <!-- Arbre -->
      <Card v-if="bracketMatches.length > 0" class="neon-panel">
        <CardHeader>
          <CardTitle>Arbre</CardTitle>
        </CardHeader>
        <CardContent class="grid gap-3">
          <TournamentMatchCard
            v-for="match in bracketMatches"
            :key="match.id"
            :match="match"
            :form="getForm(match)"
            :can-interact="canInteractWithMatch(match)"
            :is-admin="isAdmin"
            :player1-army-id="matchPlayerArmyId(match, 'player1')"
            :player2-army-id="matchPlayerArmyId(match, 'player2')"
            :status-label="matchStatusLabel(match)"
            :phase-label="phaseLabels[match.phase] ?? match.phase"
            @submit="submitMatch(match)"
            @confirm="confirmMatch(match)"
            @correct="correctMatch(match, $event)"
            @forfeit="forfeitMatch(match, $event)"
            @unplayed="markMatchUnplayed(match)"
          />
        </CardContent>
      </Card>

      <!-- Poules (classement + détail matchs au clic) -->
      <Card
        v-if="detail.pools.length > 0 && !canEditPools"
        class="neon-panel"
      >
        <CardHeader>
          <CardTitle>Poules</CardTitle>
          <CardDescription>
            Cliquez sur une poule pour afficher ses matchs.
          </CardDescription>
        </CardHeader>
        <CardContent class="grid gap-4">
          <div class="grid gap-4 md:grid-cols-2">
            <div
              v-for="pool in sortedPools"
              :key="pool.id"
              class="pool-summary"
              :class="{ 'pool-summary--active': isPoolSelected(pool.id) }"
            >
              <button
                type="button"
                class="pool-summary-header"
                @click="togglePoolDetail(pool.id)"
              >
                <h3 class="font-semibold">{{ pool.name }}</h3>
                <span class="text-xs tabular-nums text-muted-foreground">PT / PO / PS</span>
              </button>
              <ol class="space-y-1 text-sm">
                <li
                  v-for="(pp, idx) in sortedPoolPlayers(pool)"
                  :key="pp.player_name"
                  class="flex items-center justify-between gap-2"
                >
                  <span class="flex min-w-0 items-center gap-2">
                    <span class="shrink-0 tabular-nums text-muted-foreground">{{ idx + 1 }}.</span>
                    <PlayerLink
                      :name="pp.player_name"
                      :display-name="pp.player_display_name"
                    />
                    <ArmyLogo
                      v-if="poolPlayerArmyId(pp)"
                      :army-id="poolPlayerArmyId(pp)!"
                      class="shrink-0"
                    />
                  </span>
                  <span
                    class="shrink-0 tabular-nums text-muted-foreground"
                    title="Points tournoi / objectifs / survivants"
                  >
                    {{ pp.points }}/{{ pp.objectives }}/{{ pp.survivors }}
                  </span>
                </li>
              </ol>
            </div>
          </div>

          <section
            v-if="selectedPoolId && selectedPool()"
            class="pool-detail"
          >
            <div class="pool-detail-header">
              <h3 class="font-semibold">
                {{ selectedPool()!.name }} — matchs
              </h3>
              <Button
                size="sm"
                variant="ghost"
                @click="selectedPoolId = null"
              >
                Fermer
              </Button>
            </div>
            <div class="grid gap-3">
              <TournamentMatchCard
                v-for="match in poolMatchesForPool(selectedPoolId)"
                :key="match.id"
                :match="match"
                :form="getForm(match)"
                :can-interact="canInteractWithMatch(match)"
                :is-admin="isAdmin"
                :player1-army-id="matchPlayerArmyId(match, 'player1')"
                :player2-army-id="matchPlayerArmyId(match, 'player2')"
                :status-label="matchStatusLabel(match)"
                @submit="submitMatch(match)"
                @confirm="confirmMatch(match)"
                @correct="correctMatch(match, $event)"
                @forfeit="forfeitMatch(match, $event)"
                @unplayed="markMatchUnplayed(match)"
              />
              <p
                v-if="poolMatchesForPool(selectedPoolId).length === 0"
                class="rounded-lg border border-dashed p-6 text-center text-sm text-muted-foreground"
              >
                Aucun match pour cette poule.
              </p>
            </div>
          </section>
        </CardContent>
      </Card>

      <!-- Mes matchs de poule -->
      <Card
        v-if="poolMatches.length > 0"
        class="neon-panel page-panel-scroll"
      >
        <CardHeader>
          <CardTitle>Mes matchs</CardTitle>
          <CardDescription>
            Historique et saisie de vos résultats en phase de poules.
          </CardDescription>
        </CardHeader>
        <CardContent class="grid gap-3">
          <p
            v-if="!isAuthenticated"
            class="rounded-lg border border-dashed p-6 text-center text-sm text-muted-foreground"
          >
            Connectez-vous pour consulter et renseigner vos matchs.
          </p>
          <p
            v-else-if="!hasPlayer"
            class="rounded-lg border border-dashed p-6 text-center text-sm text-muted-foreground"
          >
            Votre compte n'est pas lié à un joueur du classement.
          </p>
          <template v-else>
            <TournamentMatchCard
              v-for="match in myPoolMatches"
              :key="match.id"
              :match="match"
              :form="getForm(match)"
              :can-interact="canInteractWithMatch(match)"
              :is-admin="isAdmin"
              :player1-army-id="matchPlayerArmyId(match, 'player1')"
              :player2-army-id="matchPlayerArmyId(match, 'player2')"
              :status-label="matchStatusLabel(match)"
              @submit="submitMatch(match)"
              @confirm="confirmMatch(match)"
              @correct="correctMatch(match, $event)"
              @forfeit="forfeitMatch(match, $event)"
              @unplayed="markMatchUnplayed(match)"
            />
            <p
              v-if="myPoolMatches.length === 0"
              class="rounded-lg border border-dashed p-6 text-center text-sm text-muted-foreground"
            >
              Vous n'avez pas de match de poule dans ce tournoi.
            </p>
          </template>
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
    </template>
  </div>
</template>
