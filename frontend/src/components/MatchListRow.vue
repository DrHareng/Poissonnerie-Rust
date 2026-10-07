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
  }>(),
  {
    emphasizeDefeat: false,
    showElo: false,
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
</script>

<template>
  <div
    role="link"
    tabindex="0"
    class="match-list-row"
    @click="openMatch"
    @keydown.enter.prevent="openMatch"
    @keydown.space.prevent="openMatch"
  >
    <div class="match-list-row-players">
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
    <div class="match-list-row-meta">
      <span class="tabular-nums shrink-0 self-start pt-0.5">{{ shortDate ?? '—' }}</span>
      <MatchContextCell :match="match" class="min-w-0 flex-1 text-[11px]" />
      <span
        v-if="eloLabel"
        class="ml-auto shrink-0 self-start pt-0.5 tabular-nums"
        :class="eloClass"
      >
        {{ eloLabel }}
      </span>
    </div>
  </div>
</template>
