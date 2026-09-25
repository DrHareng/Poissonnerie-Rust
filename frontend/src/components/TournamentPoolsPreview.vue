<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { ArrowUp } from '@lucide/vue'
import ArmyLogo from '@/components/ArmyLogo.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import { Badge } from '@/components/ui/badge'
import { useArmies } from '@/composables/useArmies'
import { registrationStatusLabel } from '@/lib/tournamentDisplay'
import type { Pool, PoolPlayer, RegistrationStatus, TournamentMatch } from '@/types/elo'

export type PoolStandingRegistration = {
  player_name: string
  status: RegistrationStatus
  has_army_lists?: boolean
  has_army_list_2?: boolean
  army_list_1_validated?: boolean
  army_list_2_validated?: boolean
  army_id?: number | null
}

const props = withDefaults(
  defineProps<{
    pools: Pool[]
    /** Inscriptions pour badges « en attente… » quand la sectorielle est masquée. */
    registrations?: PoolStandingRegistration[]
    /** Matchs de poule pour le compteur jouée/total. */
    matches?: TournamentMatch[]
    /** Si true, la carte poule est cliquable. */
    selectable?: boolean
    /** Nombre de premiers du classement qualifiés. */
    qualifiedPerPool?: number
    /** Masque les colonnes PO / PS (aperçu compressé, ex. accueil). */
    hideTiebreakers?: boolean
    /** Une poule par ligne au lieu de deux colonnes. */
    stacked?: boolean
  }>(),
  {
    registrations: () => [],
    matches: () => [],
    selectable: false,
    qualifiedPerPool: 0,
    hideTiebreakers: false,
    stacked: false,
  },
)

const emit = defineEmits<{
  selectPool: [poolId: number]
}>()

const { ensureLoaded, getArmy } = useArmies()

onMounted(() => {
  void ensureLoaded()
})

const registrationByPlayer = computed(() => {
  const map = new Map<string, PoolStandingRegistration>()
  for (const registration of props.registrations) {
    map.set(registration.player_name.toLowerCase(), registration)
  }
  return map
})

const sortedPools = computed(() =>
  [...props.pools].sort((a, b) => a.position - b.position),
)

function sortedPoolPlayers(pool: Pool) {
  return [...pool.players].sort(
    (a, b) =>
      b.points - a.points
      || b.objectives - a.objectives
      || b.survivors - a.survivors,
  )
}

function registrationFor(playerName: string) {
  return registrationByPlayer.value.get(playerName.toLowerCase())
}

function listsFullyValidated(reg: PoolStandingRegistration | undefined) {
  if (!reg?.has_army_lists || !reg.army_list_1_validated) return false
  if (reg.has_army_list_2 && !reg.army_list_2_validated) return false
  return true
}

function playerArmyId(pp: PoolPlayer) {
  if (pp.army_id) return pp.army_id
  const reg = registrationFor(pp.player_name)
  return reg?.army_id ?? undefined
}

function playerArmyName(pp: PoolPlayer) {
  return getArmy(playerArmyId(pp))?.name ?? null
}

function playerPendingStatus(pp: PoolPlayer): string | null {
  if (playerArmyId(pp)) return null
  const reg = registrationFor(pp.player_name)
  if (!reg) return null
  if (listsFullyValidated(reg) || reg.status === 'approved') return null
  return registrationStatusLabel(reg)
}

function playerLabel(pp: PoolPlayer) {
  return pp.player_display_name?.trim() || pp.player_name
}

function playerInitial(pp: PoolPlayer) {
  const label = playerLabel(pp)
  return label.charAt(0).toUpperCase() || '?'
}

function isPoolMatchFinished(match: TournamentMatch) {
  if (match.is_unplayed) return true
  if (match.status === 'confirmed') return true
  // Forfait déclaré : la partie ne sera pas jouée.
  if (match.is_forfeit && match.status !== 'scheduled') return true
  return false
}

function matchInvolvesPlayer(match: TournamentMatch, playerName: string) {
  const key = playerName.toLowerCase()
  return (
    match.player1?.toLowerCase() === key
    || match.player2?.toLowerCase() === key
  )
}

function playerGamesLabel(pool: Pool, playerName: string) {
  const poolMatches = props.matches.filter(
    (match) =>
      match.phase === 'pool'
      && match.pool_id === pool.id
      && matchInvolvesPlayer(match, playerName),
  )
  const total =
    poolMatches.length > 0
      ? poolMatches.length
      : Math.max(0, pool.players.length - 1)
  const played = poolMatches.filter(isPoolMatchFinished).length
  return `${played}/${total}`
}

function onSelectPool(poolId: number) {
  if (!props.selectable) return
  emit('selectPool', poolId)
}

function playerRatingLabel(pp: PoolPlayer) {
  if (pp.rating == null || Number.isNaN(pp.rating)) return null
  return String(Math.round(pp.rating))
}

function isQualifiedRank(rankIndex: number) {
  const n = props.qualifiedPerPool
  return n > 0 && rankIndex < n
}
</script>

<template>
  <div class="grid gap-3" :class="{ 'md:grid-cols-2': !stacked }">
    <div
      v-for="pool in sortedPools"
      :key="pool.id"
      class="pool-summary"
      :class="{ 'pool-summary--selectable': selectable }"
      :role="selectable ? 'button' : undefined"
      :tabindex="selectable ? 0 : undefined"
      @click="onSelectPool(pool.id)"
      @keydown.enter.prevent="onSelectPool(pool.id)"
      @keydown.space.prevent="onSelectPool(pool.id)"
    >
      <div class="pool-summary-header">
        <h3 class="font-semibold">{{ pool.name }}</h3>
      </div>
      <table class="pool-standings-table">
        <thead>
          <tr>
            <th class="pool-col-rank"></th>
            <th class="pool-col-player">Joueur</th>
            <th class="pool-col-army">Sectorielle</th>
            <th class="pool-col-elo">ELO</th>
            <th class="pool-col-played">Jouées</th>
            <th class="pool-col-stat">PT</th>
            <th v-if="!hideTiebreakers" class="pool-col-stat">PO</th>
            <th v-if="!hideTiebreakers" class="pool-col-stat">PS</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="(pp, idx) in sortedPoolPlayers(pool)"
            :key="pp.player_name"
            :class="{ 'pool-standings-row--qualified': isQualifiedRank(idx) }"
          >
            <td class="pool-col-rank">
              <span
                v-if="isQualifiedRank(idx)"
                class="pool-standings-qualified-mark"
                title="Qualifié"
                aria-label="Qualifié"
              >
                <ArrowUp aria-hidden="true" />
              </span>
            </td>
            <td class="pool-col-player">
              <span class="flex min-w-0 items-center gap-2">
                <img
                  v-if="pp.avatar_url"
                  :src="pp.avatar_url"
                  :alt="playerLabel(pp)"
                  class="pool-standings-avatar"
                />
                <span
                  v-else
                  class="pool-standings-avatar-fallback"
                  aria-hidden="true"
                >{{ playerInitial(pp) }}</span>
                <PlayerLink
                  class="min-w-0 flex-1 truncate"
                  :name="pp.player_name"
                  :display-name="pp.player_display_name"
                  @click.stop
                />
              </span>
            </td>
            <td class="pool-col-army">
              <span
                v-if="playerArmyId(pp)"
                class="flex min-w-0 items-center gap-2"
                :title="playerArmyName(pp) ?? undefined"
              >
                <ArmyLogo
                  :army-id="playerArmyId(pp)!"
                  class="shrink-0"
                />
                <span class="min-w-0 truncate">{{ playerArmyName(pp) }}</span>
              </span>
              <Badge
                v-else-if="playerPendingStatus(pp)"
                variant="outline"
                class="max-w-full truncate text-xs font-normal"
                :title="playerPendingStatus(pp) ?? undefined"
              >
                {{ playerPendingStatus(pp) }}
              </Badge>
              <span v-else class="text-muted-foreground">—</span>
            </td>
            <td class="pool-col-elo">{{ playerRatingLabel(pp) }}</td>
            <td class="pool-col-played">
              {{ playerGamesLabel(pool, pp.player_name) }}
            </td>
            <td class="pool-col-stat">{{ pp.points }}</td>
            <td v-if="!hideTiebreakers" class="pool-col-stat">{{ pp.objectives }}</td>
            <td v-if="!hideTiebreakers" class="pool-col-stat">{{ pp.survivors }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
