<script setup lang="ts">
import { ref, watch } from 'vue'
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
import ArmyLogo from '@/components/ArmyLogo.vue'
import MatchResultBadges from '@/components/MatchResultBadges.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import TournamentMatchScoreboard from '@/components/TournamentMatchScoreboard.vue'
import { Button } from '@/components/ui/button'
import { tournamentMatchScenarioPath } from '@/lib/tournamentMatchDisplay'
import { COUPE_REQUIRES_NETWORK } from '@/lib/partieOffline'
import type { TournamentMatchForm } from '@/components/TournamentMatchCard.vue'
import type { TournamentMatch } from '@/types/elo'

const props = withDefaults(
  defineProps<{
    matches: TournamentMatch[]
    isAdmin: boolean
    currentPlayerName?: string | null
    getForm: (match: TournamentMatch) => TournamentMatchForm
    canInteract: (match: TournamentMatch) => boolean
    playerArmyId: (match: TournamentMatch, slot: 'player1' | 'player2') => number | undefined
    playerHasList2?: (match: TournamentMatch, slot: 'player1' | 'player2') => boolean
    statusLabel: (match: TournamentMatch) => string
    allowUnplayed?: boolean
    listsReady?: (match: TournamentMatch) => boolean
    listsReadyMessage?: (match: TournamentMatch) => string
    isOnline?: boolean
  }>(),
  {
    allowUnplayed: true,
    currentPlayerName: null,
    isOnline: true,
  },
)

const emit = defineEmits<{
  start: [match: TournamentMatch]
  resume: [match: TournamentMatch]
  confirm: [match: TournamentMatch]
  correct: [match: TournamentMatch, form: TournamentMatchForm]
  forfeit: [match: TournamentMatch, playerName: string]
  cancelForfeit: [match: TournamentMatch]
  unplayed: [match: TournamentMatch]
}>()

const correctingMatchId = ref<number | null>(null)

const menuItemClass =
  'flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-none select-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground'

const menuItemDangerClass =
  `${menuItemClass} text-destructive data-[highlighted]:bg-destructive/15 data-[highlighted]:text-destructive`

function isCorrecting(match: TournamentMatch) {
  return correctingMatchId.value === match.id
}

function canStart(match: TournamentMatch) {
  return (
    props.canInteract(match)
    && Boolean(match.player1 && match.player2)
    && match.status === 'scheduled'
    && !match.is_forfeit
    && !match.is_unplayed
    && !match.elo_match_id
    && (props.listsReady?.(match) ?? true)
  )
}

function canResume(match: TournamentMatch) {
  return (
    props.canInteract(match)
    && Boolean(match.elo_match_id)
    && match.status === 'scheduled'
    && !match.is_forfeit
    && !match.is_unplayed
  )
}

function canConfirm(match: TournamentMatch) {
  return props.canInteract(match) && match.status === 'submitted'
}

function canCorrect(match: TournamentMatch) {
  return (
    props.isAdmin
    && (match.status === 'confirmed' || match.status === 'submitted')
    && !match.is_forfeit
    && !isCorrecting(match)
  )
}

function selfForfeitName(match: TournamentMatch) {
  const me = props.currentPlayerName?.toLowerCase()
  if (!me) return null
  if (match.player1?.toLowerCase() === me) return match.player1
  if (match.player2?.toLowerCase() === me) return match.player2
  return null
}

function showMatchOptionsMenu(match: TournamentMatch) {
  return (
    Boolean(match.player1 && match.player2)
    && match.status === 'scheduled'
    && !match.is_unplayed
    && !isCorrecting(match)
    && (Boolean(selfForfeitName(match)) || props.isAdmin)
  )
}

function showPrimaryAction(match: TournamentMatch) {
  return (
    canStart(match)
    || canResume(match)
    || canConfirm(match)
    || canCorrect(match)
    || (props.isAdmin && match.is_forfeit)
  )
}

function showStatusInActions(match: TournamentMatch) {
  return !showPrimaryAction(match) && !showMatchOptionsMenu(match)
}

function matchPlayerLabel(match: TournamentMatch, slot: 'player1' | 'player2') {
  const displayName =
    slot === 'player1' ? match.player1_display_name : match.player2_display_name
  const name = match[slot]
  return displayName || name || '?'
}

function hasScores(match: TournamentMatch) {
  return (
    match.status === 'confirmed'
    || match.status === 'submitted'
    || match.is_forfeit
    || match.is_unplayed
  )
}

function scoreColumnPendingLabel(match: TournamentMatch) {
  if (match.is_unplayed) return 'Non joué'
  if (match.is_forfeit) return 'Forfait'
  return '—'
}

function scenarioLabel(match: TournamentMatch) {
  return match.scenario_name?.trim() || match.scenario_other?.trim() || null
}

function scenarioPath(match: TournamentMatch) {
  return tournamentMatchScenarioPath(match)
}

function startCorrection(match: TournamentMatch) {
  const form = props.getForm(match)
  form.p1 = match.player1_objectives
  form.p2 = match.player2_objectives
  form.s1 = match.player1_survivors
  form.s2 = match.player2_survivors
  correctingMatchId.value = match.id
}

function saveCorrection(match: TournamentMatch) {
  const form = props.getForm(match)
  emit('correct', match, {
    p1: Number(form.p1) || 0,
    p2: Number(form.p2) || 0,
    s1: Number(form.s1) || 0,
    s2: Number(form.s2) || 0,
  })
  correctingMatchId.value = null
}

function showEditRow(match: TournamentMatch) {
  if (isCorrecting(match)) return true
  return (
    props.canInteract(match)
    && match.status === 'scheduled'
    && !match.is_unplayed
    && Boolean(props.listsReady)
    && !(props.listsReady?.(match) ?? true)
  )
}

watch(
  () => props.matches.map((m) => `${m.id}:${m.status}:${m.player1_objectives}`).join(','),
  () => {
    correctingMatchId.value = null
  },
)
</script>

<template>
  <table class="pool-matches-table">
    <thead>
      <tr>
        <th class="pool-col-player">Joueur 1</th>
        <th class="pool-col-score">Score</th>
        <th class="pool-col-player">Joueur 2</th>
        <th class="pool-col-scenario">Scénario</th>
        <th class="pool-col-admin" />
      </tr>
    </thead>
    <tbody>
      <template v-for="match in matches" :key="match.id">
        <tr>
          <td class="pool-col-player">
            <span class="flex min-w-0 items-center gap-2">
              <PlayerLink
                v-if="match.player1"
                :name="match.player1"
                :display-name="match.player1_display_name"
              />
              <span v-else class="text-muted-foreground">?</span>
              <ArmyLogo
                v-if="playerArmyId(match, 'player1')"
                :army-id="playerArmyId(match, 'player1')!"
                class="shrink-0"
              />
            </span>
          </td>
          <td class="pool-col-score">
            <div
              v-if="hasScores(match)"
              class="pool-match-result-badges"
            >
              <MatchResultBadges
                v-if="match.is_unplayed || (match.is_forfeit && !match.outcome)"
                :match="{
                  player1_objectives: 0,
                  player2_objectives: 0,
                  player1_survivors: 0,
                  player2_survivors: 0,
                  outcome: null,
                }"
                :pending-label="scoreColumnPendingLabel(match)"
                :badge-min-ch="5"
              />
              <MatchResultBadges
                v-else
                :match="match"
                :badge-min-ch="5"
              />
            </div>
            <span v-else class="text-muted-foreground">—</span>
          </td>
          <td class="pool-col-player">
            <span class="flex min-w-0 items-center gap-2">
              <PlayerLink
                v-if="match.player2"
                :name="match.player2"
                :display-name="match.player2_display_name"
              />
              <span v-else class="text-muted-foreground">?</span>
              <ArmyLogo
                v-if="playerArmyId(match, 'player2')"
                :army-id="playerArmyId(match, 'player2')!"
                class="shrink-0"
              />
            </span>
          </td>
          <td class="pool-col-scenario">
            <RouterLink
              v-if="scenarioPath(match) && scenarioLabel(match)"
              :to="scenarioPath(match)!"
              class="pool-scenario-link"
              :title="scenarioLabel(match)!"
              @click.stop
            >
              {{ scenarioLabel(match) }}
            </RouterLink>
            <span
              v-else
              :title="scenarioLabel(match) ?? undefined"
            >
              {{ scenarioLabel(match) ?? '—' }}
            </span>
          </td>
          <td class="pool-col-admin">
            <div class="pool-match-admin-actions">
              <template v-if="showStatusInActions(match)">
                <span class="pool-match-status-label">
                  {{ statusLabel(match) }}
                </span>
              </template>
              <template v-else>
                <Button
                  v-if="canStart(match)"
                  size="sm"
                  :disabled="!isOnline"
                  :title="!isOnline ? COUPE_REQUIRES_NETWORK : undefined"
                  @click="emit('start', match)"
                >
                  Démarrer
                </Button>
                <Button
                  v-else-if="canResume(match)"
                  size="sm"
                  :disabled="!isOnline"
                  :title="!isOnline ? COUPE_REQUIRES_NETWORK : undefined"
                  @click="emit('resume', match)"
                >
                  Reprendre
                </Button>
                <Button
                  v-else-if="canConfirm(match)"
                  size="sm"
                  variant="outline"
                  @click="emit('confirm', match)"
                >
                  {{ match.is_forfeit ? 'Confirmer FF' : 'Confirmer' }}
                </Button>
                <Button
                  v-else-if="canCorrect(match)"
                  size="sm"
                  variant="outline"
                  @click="startCorrection(match)"
                >
                  Corriger
                </Button>
                <Button
                  v-else-if="isAdmin && match.is_forfeit"
                  size="sm"
                  variant="outline"
                  @click="emit('cancelForfeit', match)"
                >
                  Annuler FF
                </Button>
                <DropdownMenuRoot v-if="showMatchOptionsMenu(match)">
                  <DropdownMenuTrigger as-child>
                    <Button
                      size="sm"
                      variant="outline"
                      class="pool-match-options-trigger"
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
                      class="z-50 min-w-48 rounded-md border bg-popover p-1 text-popover-foreground shadow-md"
                    >
                      <DropdownMenuItem
                        v-if="selfForfeitName(match)"
                        :class="menuItemDangerClass"
                        @select="emit('forfeit', match, selfForfeitName(match)!)"
                      >
                        Je déclare forfait
                      </DropdownMenuItem>
                      <DropdownMenuSeparator
                        v-if="selfForfeitName(match) && isAdmin"
                        class="my-1 h-px bg-border"
                      />
                      <template v-if="isAdmin">
                        <DropdownMenuItem
                          v-if="allowUnplayed && match.phase === 'pool'"
                          :class="menuItemClass"
                          @select="emit('unplayed', match)"
                        >
                          Match non joué
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          :class="menuItemDangerClass"
                          @select="emit('forfeit', match, match.player1!)"
                        >
                          FF {{ matchPlayerLabel(match, 'player1') }}
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          :class="menuItemDangerClass"
                          @select="emit('forfeit', match, match.player2!)"
                        >
                          FF {{ matchPlayerLabel(match, 'player2') }}
                        </DropdownMenuItem>
                      </template>
                    </DropdownMenuContent>
                  </DropdownMenuPortal>
                </DropdownMenuRoot>
              </template>
            </div>
          </td>
        </tr>
        <tr
          v-if="showEditRow(match)"
          class="pool-match-edit-row"
        >
          <td colspan="5">
            <div class="pool-match-edit-panel">
              <template v-if="isCorrecting(match)">
                <TournamentMatchScoreboard
                  :match="match"
                  mode="form"
                  :form="getForm(match)"
                  :player1-army-id="playerArmyId(match, 'player1')"
                  :player2-army-id="playerArmyId(match, 'player2')"
                />
                <div class="pool-match-edit-actions">
                  <Button size="sm" @click="saveCorrection(match)">
                    Enregistrer
                  </Button>
                  <Button size="sm" variant="outline" @click="correctingMatchId = null">
                    Annuler
                  </Button>
                </div>
              </template>
              <p
                v-else
                class="text-sm text-amber-600 dark:text-amber-400"
              >
                {{ listsReadyMessage?.(match) || 'Listes d’arbre manquantes.' }}
              </p>
            </div>
          </td>
        </tr>
      </template>
    </tbody>
  </table>
</template>
