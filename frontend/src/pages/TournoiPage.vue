<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Check, X } from '@lucide/vue'
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
  TournamentPhase,
  TournamentRegistration,
} from '@/types/elo'
import { useAuth } from '@/composables/useAuth'
import ArmyLogo from '@/components/ArmyLogo.vue'
import BracketTree from '@/components/BracketTree.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import PlayerPicker from '@/components/PlayerPicker.vue'
import SectorialPicker from '@/components/SectorialPicker.vue'
import PoolMatchesTable from '@/components/PoolMatchesTable.vue'
import TournamentMatchCard from '@/components/TournamentMatchCard.vue'
import type { TournamentMatchForm } from '@/components/TournamentMatchCard.vue'
import { formatRegistrationSummary, topFourDisplayRows } from '@/lib/tournamentDisplay'
import { phaseLabels } from '@/lib/tournamentPhase'
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

const myTournamentMatches = computed(() => {
  if (!player.value || !detail.value) return []
  const name = player.value.name.toLowerCase()
  return detail.value.matches.filter(
    (match) =>
      match.player1?.toLowerCase() === name
      || match.player2?.toLowerCase() === name,
  )
})

type TournamentTabId = 'resume' | 'arbre' | 'poules' | 'inscriptions' | 'admin'

const activeTab = ref<TournamentTabId>('resume')

const showArbreTab = computed(
  () => bracketMatches.value.length > 0 || canEditBracket.value,
)

const showPoulesTab = computed(
  () => (detail.value?.pools.length ?? 0) > 0 || canEditPools.value,
)

const tournamentTabs = computed(() => {
  const tabs: { id: TournamentTabId; label: string }[] = [
    { id: 'resume', label: 'Résumé' },
  ]
  if (showArbreTab.value) {
    tabs.push({ id: 'arbre', label: "L'arbre" })
  }
  if (showPoulesTab.value) {
    tabs.push({ id: 'poules', label: 'Phase de poules' })
  }
  tabs.push({ id: 'inscriptions', label: 'Inscriptions' })
  if (isAdmin.value) {
    tabs.push({ id: 'admin', label: 'Administration' })
  }
  return tabs
})

function setActiveTab(tab: TournamentTabId) {
  activeTab.value = tab
}

function poolMatchesForPool(poolId: number) {
  return poolMatches.value.filter((match) => match.pool_id === poolId)
}

function selectPool(poolId: number) {
  selectedPoolId.value = poolId
}

function clearPoolSelection() {
  selectedPoolId.value = null
}

function selectedPool() {
  return sortedPools.value.find((pool) => pool.id === selectedPoolId.value) ?? null
}

const bracketMatches = computed(() =>
  detail.value?.matches.filter((m) => m.phase !== 'pool') ?? [],
)

const bracketPhaseOrder: TournamentPhase[] = [
  'round_of_16',
  'quarter',
  'semi',
  'final',
]

const bracketPhases = computed(() =>
  bracketPhaseOrder
    .map((phase) => ({
      phase,
      label: phaseLabels[phase],
      matches: bracketMatches.value
        .filter((match) => match.phase === phase)
        .sort((a, b) => (a.bracket_slot ?? 0) - (b.bracket_slot ?? 0)),
    }))
    .filter((section) => section.matches.length > 0),
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
  () =>
    sortedPools.value.length === 4
    && detail.value?.bracket_format !== 'quarters_direct',
)

const isQuartersDirectFormat = computed(
  () => detail.value?.bracket_format === 'quarters_direct',
)

const isFullRoundOf16Format = computed(
  () =>
    detail.value?.bracket_format === 'round_of_16_full'
    && (detail.value?.pool_count ?? 0) >= 8,
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
  const topN = isRoundOf16Format.value ? 3 : 2
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

  if (isRoundOf16Format.value) {
    return [
      { bracket_slot: 0, player1: second(3), player2: third(1), quarter_player1: first(0) },
      { bracket_slot: 1, player1: second(2), player2: third(0), quarter_player1: first(1) },
      { bracket_slot: 2, player1: second(0), player2: third(3), quarter_player1: first(2) },
      { bracket_slot: 3, player1: second(1), player2: third(2), quarter_player1: first(3) },
    ]
  }

  if (isQuartersDirectFormat.value) {
    return [
      { bracket_slot: 0, player1: second(3), player2: first(0) },
      { bracket_slot: 1, player1: second(2), player2: first(1) },
      { bracket_slot: 2, player1: second(0), player2: first(2) },
      { bracket_slot: 3, player1: second(1), player2: first(3) },
    ]
  }

  if (isFullRoundOf16Format.value) {
    const firsts = ranked.map((pool) => pool[0]?.player_name)
    const seconds = ranked.map((pool) => pool[1]?.player_name)
    const pairings = [
      [0, 1], [1, 0], [2, 3], [3, 2], [4, 5], [5, 4], [6, 7], [7, 6],
    ] as const
    return pairings.map(([firstIndex, secondIndex], bracket_slot) => ({
      bracket_slot,
      player1: firsts[firstIndex],
      player2: seconds[secondIndex],
    }))
  }

  return []
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
        if (isRoundOf16Format.value) {
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

async function saveDefaultBracket() {
  manualBracket.value = defaultBracketSlots()
  await saveManualBracket()
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
watch(tournamentTabs, (tabs) => {
  if (!tabs.some((tab) => tab.id === activeTab.value)) {
    activeTab.value = 'resume'
  }
})
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
    <div v-if="loading" class="text-muted-foreground">Chargement...</div>

    <template v-else-if="detail">
      <section class="page-header">
        <h1 class="page-title">{{ detail.name }}</h1>
        <p class="page-description">
          {{ detail.display_status }}
          — {{ formatRegistrationSummary(detail.approved_count, detail.waitlist_count) }}
        </p>
      </section>

      <nav class="tournament-tabs" aria-label="Sections du tournoi">
        <button
          v-for="tab in tournamentTabs"
          :key="tab.id"
          type="button"
          class="tournament-tab"
          :class="{ 'tournament-tab--active': activeTab === tab.id }"
          @click="setActiveTab(tab.id)"
        >
          {{ tab.label }}
        </button>
      </nav>

      <div class="tournament-tab-panels page-panel-scroll">
        <template v-if="activeTab === 'resume'">
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

          <Card class="neon-panel">
            <CardHeader>
              <CardTitle>Mes matchs</CardTitle>
              <CardDescription>
                Vos parties dans ce tournoi.
              </CardDescription>
            </CardHeader>
            <CardContent class="grid gap-3">
              <p
                v-if="!isAuthenticated"
                class="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground"
              >
                Connectez-vous pour consulter et renseigner vos matchs.
              </p>
              <p
                v-else-if="!hasPlayer"
                class="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground"
              >
                Votre compte n'est pas lié à un joueur du classement.
              </p>
              <template v-else>
                <TournamentMatchCard
                  v-for="match in myTournamentMatches"
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
                <p
                  v-if="myTournamentMatches.length === 0"
                  class="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground"
                >
                  Vous n'avez pas de match dans ce tournoi.
                </p>
              </template>
            </CardContent>
          </Card>
        </template>

        <template v-else-if="activeTab === 'arbre'">
          <Card v-if="canEditBracket" class="neon-panel">
            <CardHeader>
              <CardTitle>Configurer l'arbre</CardTitle>
              <CardDescription v-if="isRoundOf16Format">
                4 barrages : 2e vs 3e de poules croisées. Les 1ers de poule attendent en quart de finale.
              </CardDescription>
              <CardDescription v-else-if="isQuartersDirectFormat">
                Quarts directs : 1er d'une poule vs 2e d'une autre poule.
              </CardDescription>
              <CardDescription v-else>
                Définissez manuellement les affrontements du premier tour de l'arbre.
              </CardDescription>
            </CardHeader>
            <CardContent class="grid gap-3 md:grid-cols-2">
              <div
                v-for="slot in manualBracket"
                :key="slot.bracket_slot"
                class="rounded-lg border p-3"
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

              <div class="flex flex-wrap items-center justify-between gap-3 border-t pt-3 md:col-span-2">
                <Button
                  size="sm"
                  variant="outline"
                  @click="manualBracket = defaultBracketSlots()"
                >
                  Réinitialiser les appariements
                </Button>
                <div class="flex flex-wrap gap-2">
                  <Button
                    v-if="isRoundOf16Format || isQuartersDirectFormat"
                    size="sm"
                    variant="outline"
                    :disabled="savingBracket"
                    @click="saveDefaultBracket"
                  >
                    Arbre par défaut
                  </Button>
                  <Button :disabled="savingBracket" @click="saveManualBracket">
                    {{ savingBracket ? 'Enregistrement...' : 'Enregistrer l\'arbre' }}
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card v-if="bracketMatches.length > 0" class="neon-panel">
            <CardHeader>
              <CardTitle>Arbre</CardTitle>
            </CardHeader>
            <CardContent class="grid gap-6">
              <BracketTree :matches="bracketMatches" />
              <section
                v-for="section in bracketPhases"
                :key="section.phase"
                class="bracket-phase-section"
              >
                <h3 class="bracket-phase-title">{{ section.label }}</h3>
                <PoolMatchesTable
                  :matches="section.matches"
                  :is-admin="isAdmin"
                  :get-form="getForm"
                  :can-interact="canInteractWithMatch"
                  :player-army-id="matchPlayerArmyId"
                  :status-label="matchStatusLabel"
                  :allow-unplayed="false"
                  @submit="submitMatch"
                  @confirm="confirmMatch"
                  @correct="correctMatch"
                  @forfeit="forfeitMatch"
                  @unplayed="markMatchUnplayed"
                />
              </section>
            </CardContent>
          </Card>
        </template>

        <template v-else-if="activeTab === 'poules'">
          <Card v-if="canEditPools" class="neon-panel">
            <CardHeader>
              <CardTitle>Configurer les poules</CardTitle>
              <CardDescription>
                Répartissez manuellement les joueurs validés dans chaque poule (6 max.).
              </CardDescription>
            </CardHeader>
            <CardContent class="grid gap-3 md:grid-cols-2">
              <div
                v-for="(pool, poolIndex) in manualPools"
                :key="pool.position"
                class="rounded-lg border p-3"
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

              <div class="flex flex-wrap items-center justify-between gap-3 border-t pt-3 md:col-span-2">
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

          <Card
            v-if="detail.pools.length > 0 && !canEditPools"
            class="neon-panel"
          >
            <CardHeader>
              <nav
                v-if="selectedPoolId && selectedPool()"
                class="pool-breadcrumb"
                aria-label="Navigation des poules"
              >
                <button type="button" class="pool-breadcrumb-link" @click="clearPoolSelection">
                  Poules
                </button>
                <span class="pool-breadcrumb-sep" aria-hidden="true">›</span>
                <span class="pool-breadcrumb-current">{{ selectedPool()!.name }}</span>
              </nav>
              <CardTitle v-else>
                Poules
              </CardTitle>
              <CardDescription v-if="!selectedPoolId">
                Cliquez sur une poule pour afficher ses matchs.
              </CardDescription>
            </CardHeader>
            <CardContent class="grid gap-3">
              <div v-if="!selectedPoolId" class="grid gap-3 md:grid-cols-2">
                <div
                  v-for="pool in sortedPools"
                  :key="pool.id"
                  class="pool-summary"
                >
                  <button
                    type="button"
                    class="pool-summary-header"
                    @click="selectPool(pool.id)"
                  >
                    <h3 class="font-semibold">{{ pool.name }}</h3>
                  </button>
                  <table class="pool-standings-table">
                    <thead>
                      <tr>
                        <th class="pool-col-rank">#</th>
                        <th class="pool-col-player">Joueur</th>
                        <th class="pool-col-stat">PT</th>
                        <th class="pool-col-stat">PO</th>
                        <th class="pool-col-stat">PS</th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr
                        v-for="(pp, idx) in sortedPoolPlayers(pool)"
                        :key="pp.player_name"
                      >
                        <td class="pool-col-rank text-muted-foreground">{{ idx + 1 }}</td>
                        <td class="pool-col-player">
                          <span class="flex min-w-0 items-center gap-2">
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
                        </td>
                        <td class="pool-col-stat">{{ pp.points }}</td>
                        <td class="pool-col-stat">{{ pp.objectives }}</td>
                        <td class="pool-col-stat">{{ pp.survivors }}</td>
                      </tr>
                    </tbody>
                  </table>
                </div>
              </div>

              <section
                v-else-if="selectedPool()"
                class="pool-detail"
              >
                <PoolMatchesTable
                  v-if="poolMatchesForPool(selectedPoolId!).length > 0"
                  :matches="poolMatchesForPool(selectedPoolId!)"
                  :is-admin="isAdmin"
                  :get-form="getForm"
                  :can-interact="canInteractWithMatch"
                  :player-army-id="matchPlayerArmyId"
                  :status-label="matchStatusLabel"
                  @submit="submitMatch"
                  @confirm="confirmMatch"
                  @correct="correctMatch"
                  @forfeit="forfeitMatch"
                  @unplayed="markMatchUnplayed"
                />
                <p
                  v-else
                  class="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground"
                >
                  Aucun match pour cette poule.
                </p>
              </section>
            </CardContent>
          </Card>
        </template>

        <template v-else-if="activeTab === 'inscriptions'">
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
            <CardContent class="grid gap-3">
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

        <template v-else-if="activeTab === 'admin'">
          <Card class="neon-panel-accent">
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

          <Card v-if="pendingRegistrations.length > 0" class="neon-panel">
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
        </template>
      </div>
    </template>
  </div>
</template>
