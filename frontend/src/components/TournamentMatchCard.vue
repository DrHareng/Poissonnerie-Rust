<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Settings2 } from '@lucide/vue'
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from 'reka-ui'
import { RouterLink } from 'vue-router'
import TournamentMatchScoreboard from '@/components/TournamentMatchScoreboard.vue'
import ArmyLogo from '@/components/ArmyLogo.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import { Button } from '@/components/ui/button'
import {
  formatMatchRecordedDate,
  tournamentMatchScenarioPath,
} from '@/lib/tournamentMatchDisplay'
import { COUPE_REQUIRES_NETWORK } from '@/lib/partieOffline'
import type { TournamentMatch } from '@/types/elo'

export interface TournamentMatchForm {
  p1: number
  p2: number
  s1: number
  s2: number
  list1?: number
  list2?: number
}

const props = withDefaults(
  defineProps<{
    match: TournamentMatch
    form: TournamentMatchForm
    canInteract: boolean
    isAdmin: boolean
    /** Nom du joueur connecté (pour forfait soi-même). */
    currentPlayerName?: string | null
    player1ArmyId?: number
    player2ArmyId?: number
    player1HasList2?: boolean
    player2HasList2?: boolean
    statusLabel: string
    phaseLabel?: string
    compact?: boolean
    /** Affiche VS adversaire (sans mon pseudo) + scénario en dessous. */
    opponentFocused?: boolean
    listsReady?: boolean
    listsReadyMessage?: string
    isOnline?: boolean
  }>(),
  {
    compact: false,
    opponentFocused: false,
    listsReady: true,
    listsReadyMessage: '',
    player1HasList2: false,
    player2HasList2: false,
    currentPlayerName: null,
    isOnline: true,
  },
)

const emit = defineEmits<{
  start: []
  resume: []
  confirm: []
  correct: [form: TournamentMatchForm]
  forfeit: [playerName: string]
  cancelForfeit: []
  unplayed: []
}>()

const correcting = ref(false)

const hasBothPlayers = computed(
  () => Boolean(props.match.player1 && props.match.player2),
)

const isPoolMatch = computed(() => props.match.phase === 'pool')

const showScoresView = computed(
  () =>
    (props.match.status === 'confirmed'
      || props.match.status === 'submitted'
      || props.match.is_unplayed)
    && !correcting.value,
)

const scoreboardMode = computed(() => {
  if (showScoresView.value) return 'scores' as const
  return 'players' as const
})

const canStart = computed(
  () =>
    props.canInteract
    && hasBothPlayers.value
    && props.match.status === 'scheduled'
    && !props.match.is_forfeit
    && !props.match.is_unplayed
    && props.listsReady
    && !props.match.elo_match_id,
)

const canResume = computed(
  () =>
    props.canInteract
    && Boolean(props.match.elo_match_id)
    && props.match.status === 'scheduled'
    && !props.match.is_forfeit
    && !props.match.is_unplayed,
)

const canConfirm = computed(
  () => props.canInteract && props.match.status === 'submitted',
)

const canCorrect = computed(
  () =>
    props.isAdmin
    && (props.match.status === 'confirmed' || props.match.status === 'submitted')
    && !props.match.is_forfeit
    && !correcting.value,
)

const canCancelForfeit = computed(
  () => props.isAdmin && props.match.is_forfeit,
)

const scenarioPath = computed(() => tournamentMatchScenarioPath(props.match))
const scenarioLabel = computed(
  () => props.match.scenario_name?.trim() || null,
)

const selfForfeitName = computed(() => {
  const me = props.currentPlayerName?.toLowerCase()
  if (!me) return null
  if (props.match.player1?.toLowerCase() === me) return props.match.player1
  if (props.match.player2?.toLowerCase() === me) return props.match.player2
  return null
})

const opponentSlot = computed<'player1' | 'player2' | null>(() => {
  if (!props.opponentFocused) return null
  const me = props.currentPlayerName?.toLowerCase()
  if (!me) return null
  if (props.match.player1?.toLowerCase() === me) return 'player2'
  if (props.match.player2?.toLowerCase() === me) return 'player1'
  return null
})

const showOpponentFocus = computed(
  () => props.opponentFocused && opponentSlot.value != null && !correcting.value,
)

const opponentName = computed(() => {
  const slot = opponentSlot.value
  if (!slot) return null
  return props.match[slot]
})

const opponentDisplayName = computed(() => {
  const slot = opponentSlot.value
  if (!slot) return null
  return slot === 'player1'
    ? props.match.player1_display_name
    : props.match.player2_display_name
})

const opponentArmyId = computed(() => {
  const slot = opponentSlot.value
  if (slot === 'player1') return props.player1ArmyId
  if (slot === 'player2') return props.player2ArmyId
  return undefined
})

const showMatchOptionsMenu = computed(
  () =>
    hasBothPlayers.value
    && props.match.status === 'scheduled'
    && !props.match.is_unplayed
    && !correcting.value
    && (Boolean(selfForfeitName.value) || props.isAdmin),
)

const showActionsRow = computed(
  () =>
    hasBothPlayers.value
    && (correcting.value
      || canConfirm.value
      || (!props.listsReady && props.match.status === 'scheduled' && props.canInteract)),
)

const menuItemClass =
  'flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-none select-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground'

const menuItemDangerClass =
  `${menuItemClass} text-destructive data-[highlighted]:bg-destructive/15 data-[highlighted]:text-destructive`

watch(
  () => [
    props.match.player1_objectives,
    props.match.player2_objectives,
    props.match.player1_survivors,
    props.match.player2_survivors,
    props.match.status,
  ],
  () => {
    correcting.value = false
  },
)

function startCorrection() {
  props.form.p1 = props.match.player1_objectives
  props.form.p2 = props.match.player2_objectives
  props.form.s1 = props.match.player1_survivors
  props.form.s2 = props.match.player2_survivors
  props.form.list1 = props.player1HasList2
    ? (props.match.player1_list_slot ?? undefined)
    : 1
  props.form.list2 = props.player2HasList2
    ? (props.match.player2_list_slot ?? undefined)
    : 1
  correcting.value = true
}

function cancelCorrection() {
  correcting.value = false
}

const canSaveCorrection = computed(() => {
  const list1 = props.player1HasList2 ? props.form.list1 : 1
  const list2 = props.player2HasList2 ? props.form.list2 : 1
  return (
    (list1 === 1 || list1 === 2)
    && (list2 === 1 || list2 === 2)
    && (list1 !== 2 || props.player1HasList2)
    && (list2 !== 2 || props.player2HasList2)
  )
})

function saveCorrection() {
  if (!canSaveCorrection.value) return
  emit('correct', {
    p1: Number(props.form.p1) || 0,
    p2: Number(props.form.p2) || 0,
    s1: Number(props.form.s1) || 0,
    s2: Number(props.form.s2) || 0,
    list1: props.player1HasList2 ? props.form.list1 : 1,
    list2: props.player2HasList2 ? props.form.list2 : 1,
  })
}

function matchPlayerLabel(slot: 'player1' | 'player2') {
  const name = props.match[slot]
  const displayName =
    slot === 'player1'
      ? props.match.player1_display_name
      : props.match.player2_display_name
  return displayName || name || '?'
}
</script>

<template>
  <div
    class="tournament-match-card"
    :class="{ 'tournament-match-card--compact': compact }"
  >
    <template v-if="showOpponentFocus">
      <div class="tournament-match-opponent-focus">
        <div class="tournament-match-opponent-row">
          <span class="tournament-match-vs">VS</span>
          <ArmyLogo
            v-if="opponentArmyId"
            :army-id="opponentArmyId"
            class="shrink-0"
          />
          <PlayerLink
            v-if="opponentName"
            :name="opponentName"
            :display-name="opponentDisplayName"
            class="min-w-0 font-medium"
          />
          <span v-else class="font-medium text-muted-foreground">?</span>
          <div class="tournament-match-meta-status-group ml-auto">
            <Button
              v-if="canStart"
              size="sm"
              class="tournament-match-correct-btn"
              :disabled="!isOnline"
              :title="!isOnline ? COUPE_REQUIRES_NETWORK : undefined"
              @click="emit('start')"
            >
              Démarrer
            </Button>
            <Button
              v-else-if="canResume"
              size="sm"
              class="tournament-match-correct-btn"
              :disabled="!isOnline"
              :title="!isOnline ? COUPE_REQUIRES_NETWORK : undefined"
              @click="emit('resume')"
            >
              Reprendre
            </Button>
            <Button
              v-else-if="canCorrect"
              size="sm"
              variant="outline"
              class="tournament-match-correct-btn"
              @click="startCorrection"
            >
              Corriger
            </Button>
            <Button
              v-else-if="canCancelForfeit"
              size="sm"
              variant="outline"
              class="tournament-match-correct-btn"
              @click="emit('cancelForfeit')"
            >
              Annuler forfait
            </Button>
            <span
              v-else-if="!showMatchOptionsMenu"
              class="tournament-match-meta-status"
            >
              {{ statusLabel }}
            </span>
            <DropdownMenuRoot v-if="showMatchOptionsMenu">
              <DropdownMenuTrigger as-child>
                <Button
                  size="sm"
                  variant="outline"
                  class="tournament-match-options-trigger"
                  title="Options du match"
                  aria-label="Options du match"
                >
                  <Settings2 class="size-3.5" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuPortal>
                <DropdownMenuContent
                  align="end"
                  :side-offset="6"
                  class="tournament-match-options-menu z-50 min-w-48 rounded-md border bg-popover p-1 text-popover-foreground shadow-md"
                >
                  <DropdownMenuItem
                    v-if="selfForfeitName"
                    :class="menuItemDangerClass"
                    @select="emit('forfeit', selfForfeitName)"
                  >
                    Je déclare forfait
                  </DropdownMenuItem>
                  <DropdownMenuSeparator
                    v-if="selfForfeitName && isAdmin"
                    class="my-1 h-px bg-border"
                  />
                  <template v-if="isAdmin">
                    <DropdownMenuItem
                      v-if="isPoolMatch"
                      :class="menuItemClass"
                      @select="emit('unplayed')"
                    >
                      Match non joué
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      :class="menuItemDangerClass"
                      @select="emit('forfeit', match.player1!)"
                    >
                      FF {{ matchPlayerLabel('player1') }}
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      :class="menuItemDangerClass"
                      @select="emit('forfeit', match.player2!)"
                    >
                      FF {{ matchPlayerLabel('player2') }}
                    </DropdownMenuItem>
                  </template>
                </DropdownMenuContent>
              </DropdownMenuPortal>
            </DropdownMenuRoot>
          </div>
        </div>
        <div class="tournament-match-meta">
          <span class="tournament-match-meta-phase">Scénario</span>
          <RouterLink
            v-if="scenarioPath && scenarioLabel"
            :to="scenarioPath"
            class="tournament-match-meta-scenario tournament-match-meta-scenario--link"
            :title="scenarioLabel"
            @click.stop
          >
            {{ scenarioLabel }}
          </RouterLink>
          <span
            v-else
            class="tournament-match-meta-scenario"
            :class="{ 'tournament-match-meta-empty': !scenarioLabel }"
            :title="scenarioLabel ?? undefined"
          >
            {{ scenarioLabel ?? '—' }}
          </span>
        </div>
      </div>
    </template>

    <template v-else>
      <div class="tournament-match-meta">
        <span
          class="tournament-match-meta-phase"
          :class="{ 'tournament-match-meta-empty': !phaseLabel }"
        >
          {{ phaseLabel ?? '—' }}
        </span>
        <RouterLink
          v-if="scenarioPath && scenarioLabel"
          :to="scenarioPath"
          class="tournament-match-meta-scenario tournament-match-meta-scenario--link"
          :title="scenarioLabel"
          @click.stop
        >
          {{ scenarioLabel }}
        </RouterLink>
        <span
          v-else
          class="tournament-match-meta-scenario"
          :class="{ 'tournament-match-meta-empty': !scenarioLabel }"
          :title="scenarioLabel ?? undefined"
        >
          {{ scenarioLabel ?? '—' }}
        </span>
        <span
          v-if="!compact"
          class="tournament-match-meta-date"
          :class="{ 'tournament-match-meta-empty': !match.played_at }"
        >
          {{ match.played_at ? (formatMatchRecordedDate(match.played_at) ?? '—') : '—' }}
        </span>
        <div class="tournament-match-meta-status-group">
          <Button
            v-if="canStart"
            size="sm"
            class="tournament-match-correct-btn"
            :disabled="!isOnline"
            :title="!isOnline ? COUPE_REQUIRES_NETWORK : undefined"
            @click="emit('start')"
          >
            Démarrer
          </Button>
          <Button
            v-else-if="canResume"
            size="sm"
            class="tournament-match-correct-btn"
            :disabled="!isOnline"
            :title="!isOnline ? COUPE_REQUIRES_NETWORK : undefined"
            @click="emit('resume')"
          >
            Reprendre
          </Button>
          <Button
            v-else-if="canCorrect"
            size="sm"
            variant="outline"
            class="tournament-match-correct-btn"
            @click="startCorrection"
          >
            Corriger
          </Button>
          <Button
            v-else-if="canCancelForfeit"
            size="sm"
            variant="outline"
            class="tournament-match-correct-btn"
            @click="emit('cancelForfeit')"
          >
            Annuler forfait
          </Button>
          <span
            v-else-if="!showMatchOptionsMenu"
            class="tournament-match-meta-status"
          >
            {{ statusLabel }}
          </span>
          <DropdownMenuRoot v-if="showMatchOptionsMenu">
            <DropdownMenuTrigger as-child>
              <Button
                size="sm"
                variant="outline"
                class="tournament-match-options-trigger"
                title="Options du match"
                aria-label="Options du match"
              >
                <Settings2 class="size-3.5" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuPortal>
              <DropdownMenuContent
                align="end"
                :side-offset="6"
                class="tournament-match-options-menu z-50 min-w-48 rounded-md border bg-popover p-1 text-popover-foreground shadow-md"
              >
                <DropdownMenuItem
                  v-if="selfForfeitName"
                  :class="menuItemDangerClass"
                  @select="emit('forfeit', selfForfeitName)"
                >
                  Je déclare forfait
                </DropdownMenuItem>
                <DropdownMenuSeparator
                  v-if="selfForfeitName && isAdmin"
                  class="my-1 h-px bg-border"
                />
                <template v-if="isAdmin">
                  <DropdownMenuItem
                    v-if="isPoolMatch"
                    :class="menuItemClass"
                    @select="emit('unplayed')"
                  >
                    Match non joué
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    :class="menuItemDangerClass"
                    @select="emit('forfeit', match.player1!)"
                  >
                    FF {{ matchPlayerLabel('player1') }}
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    :class="menuItemDangerClass"
                    @select="emit('forfeit', match.player2!)"
                  >
                    FF {{ matchPlayerLabel('player2') }}
                  </DropdownMenuItem>
                </template>
              </DropdownMenuContent>
            </DropdownMenuPortal>
          </DropdownMenuRoot>
        </div>
      </div>

      <div
        v-if="!correcting"
        class="tournament-match-layout"
      >
        <TournamentMatchScoreboard
          :match="match"
          :mode="scoreboardMode"
          :form="form"
          :compact="compact"
          :player1-army-id="player1ArmyId"
          :player2-army-id="player2ArmyId"
          :player1-has-list2="player1HasList2"
          :player2-has-list2="player2HasList2"
        />
      </div>
    </template>

    <div
      v-if="showActionsRow"
      class="tournament-match-actions"
      :class="{ 'mt-2 border-t-0 pt-0': showOpponentFocus && !correcting && !canConfirm }"
    >
      <template v-if="correcting">
        <TournamentMatchScoreboard
          class="w-full"
          :match="match"
          mode="form"
          :form="form"
          :compact="compact"
          :player1-army-id="player1ArmyId"
          :player2-army-id="player2ArmyId"
          :player1-has-list2="player1HasList2"
          :player2-has-list2="player2HasList2"
        />
        <Button
          size="sm"
          :disabled="!canSaveCorrection"
          @click="saveCorrection"
        >
          Enregistrer
        </Button>
        <Button size="sm" variant="outline" @click="cancelCorrection">
          Annuler
        </Button>
      </template>
      <template v-else>
        <p
          v-if="!listsReady && match.status === 'scheduled'"
          class="w-full text-sm text-amber-600 dark:text-amber-400"
        >
          {{ listsReadyMessage || 'Listes d’arbre manquantes.' }}
        </p>

        <Button
          v-if="canConfirm"
          size="sm"
          variant="outline"
          @click="emit('confirm')"
        >
          {{ match.is_forfeit ? 'Confirmer le forfait' : 'Confirmer' }}
        </Button>
      </template>
    </div>
  </div>
</template>
