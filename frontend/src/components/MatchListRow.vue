<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import type { MatchRecord } from '@/types/elo'
import ArmyLogo from '@/components/ArmyLogo.vue'
import ArmyListQuickActions from '@/components/ArmyListQuickActions.vue'
import MatchContextCell from '@/components/MatchContextCell.vue'
import MatchResultBadges from '@/components/MatchResultBadges.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import { matchCountsForElo } from '@/lib/matchElo'
import { playerMatchEloDelta } from '@/lib/matchPlayerPerspective'

const props = withDefaults(
  defineProps<{
    match: MatchRecord
    badgeMinCh?: number
    emphasizeDefeat?: boolean
    showElo?: boolean
    /** Accueil : phase seule, scénario coupé à 10 caractères, seul le bouton Army. */
    compact?: boolean
  }>(),
  {
    emphasizeDefeat: false,
    showElo: false,
    compact: false,
  },
)

const router = useRouter()

const shortDate = computed(() => {
  const timestamp = props.match.recorded_at
  if (!timestamp || timestamp < 31_536_000) return null
  const date = new Date(timestamp * 1000)
  if (date.getFullYear() === 1970) return null
  return new Intl.DateTimeFormat('fr-FR', {
    day: '2-digit',
    month: '2-digit',
  }).format(date)
})

const eloLabel = computed(() => {
  if (!props.showElo || !matchCountsForElo(props.match.counts_for_elo)) return null
  const delta = playerMatchEloDelta(props.match)
  if (delta == null) return null
  const deltaText = delta > 0 ? `+${Math.round(delta)}` : String(Math.round(delta))
  return `${Math.round(props.match.player1_old)} → ${Math.round(props.match.player1_new)} (${deltaText})`
})

const eloClass = computed(() => {
  const delta = playerMatchEloDelta(props.match)
  if (delta != null && delta < 0) return 'match-elo-delta--loss'
  if (delta != null && delta > 0) return 'match-elo-delta--gain'
  return undefined
})

function openMatch() {
  void router.push({ name: 'match', params: { id: String(props.match.id) } })
}

/** Largeur commune des pastilles, héritée par la grille des joueurs. */
const scoreTrackStyle = computed(() => {
  if (props.badgeMinCh == null) return undefined
  return {
    '--match-score-track': `calc(${props.badgeMinCh * 2}ch + 2.5rem)`,
  }
})
</script>

<template>
  <div
    role="link"
    tabindex="0"
    class="match-list-row lg:col-span-full lg:row-span-1 lg:grid lg:grid-cols-subgrid lg:items-center lg:gap-x-3 lg:px-0"
    :style="scoreTrackStyle"
    @click="openMatch"
    @keydown.enter.prevent="openMatch"
    @keydown.space.prevent="openMatch"
  >
    <div class="match-list-row-players lg:col-span-3 lg:col-start-2 lg:row-start-1 lg:min-w-0 lg:grid-cols-subgrid lg:gap-x-3">
      <div class="flex min-w-0 items-center justify-end gap-1">
        <PlayerLink
          :name="match.player1"
          :display-name="match.player1_display_name"
          :adversaire="match.adversaire"
          class="truncate text-xs sm:text-sm"
        />
        <ArmyLogo :army-id="match.player1_army_id" />
        <ArmyListQuickActions
          :code="match.player1_army_list_code"
          icon-only
          :open-only="compact"
          class="shrink-0"
          @click.stop
        />
      </div>
      <div class="home-match-score shrink-0 [&_.mx-auto]:mx-0 [&_[data-slot=badge]]:h-5 [&_[data-slot=badge]]:px-1.5 [&_[data-slot=badge]]:text-[10px] [&_[data-slot=badge]]:leading-none">
        <MatchResultBadges
          :match="match"
          :badge-min-ch="badgeMinCh"
          :emphasize-defeat="emphasizeDefeat"
        />
      </div>
      <div class="flex min-w-0 items-center gap-1">
        <ArmyLogo :army-id="match.player2_army_id" />
        <ArmyListQuickActions
          :code="match.player2_army_list_code"
          icon-only
          :open-only="compact"
          class="shrink-0"
          @click.stop
        />
        <PlayerLink
          :name="match.player2"
          :display-name="match.player2_display_name"
          :adversaire="match.adversaire"
          class="truncate text-xs sm:text-sm"
        />
      </div>
    </div>
    <div class="match-list-row-meta lg:contents">
      <span class="match-list-row-date w-14 shrink-0 self-start pt-0.5 tabular-nums lg:col-start-1 lg:row-start-1 lg:w-auto lg:self-center lg:pt-0">{{ shortDate ?? '—' }}</span>
      <MatchContextCell
        :match="match"
        class="match-list-row-context min-w-0 flex-1 text-[11px] lg:col-start-5 lg:row-start-1 lg:w-max lg:max-w-none lg:min-w-max lg:flex-none"
        :phase-only="compact"
        :tournament-name-limit="compact ? undefined : 15"
        :scenario-name-limit="compact ? 10 : undefined"
      />
      <span
        v-if="eloLabel"
        class="match-list-row-elo ml-auto shrink-0 self-start pt-0.5 tabular-nums lg:col-start-6 lg:ml-0 lg:self-center lg:pt-0"
        :class="eloClass"
      >
        {{ eloLabel }}
      </span>
    </div>
  </div>
</template>
