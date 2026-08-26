<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { Swords } from '@lucide/vue'
import { COMBAT_ESPRIT_SLUG } from '@/lib/combatEspritDraft'
import { completeMatch, submitTournamentFromPartie } from '@/lib/api'
import type { PartiePlayerSlot, PartieScenario, PartieScores } from '@/composables/usePartieFlow'
import type { MatchOutcome } from '@/types/elo'
import { useAuth } from '@/composables/useAuth'
import { externalHref } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

const props = defineProps<{
  matchId: number
  player1: PartiePlayerSlot
  player2: PartiePlayerSlot
  scenario: PartieScenario
  scores: PartieScores
  resolvedOutcome: MatchOutcome
  /** Mode tournoi : listes obligatoires + soumission tournoi. */
  tournamentMatchId?: number | null
  player1HasList2?: boolean
  player2HasList2?: boolean
  listLabel?: string
  tournamentId?: number | null
}>()

const emit = defineEmits<{
  back: []
  'update:scores': [scores: PartieScores]
  recorded: []
}>()

const router = useRouter()
const { isAuthenticated, login } = useAuth()
const submitting = ref(false)
const list1 = ref<number | undefined>(undefined)
const list2 = ref<number | undefined>(undefined)

watch(
  () => [props.player1HasList2, props.player2HasList2] as const,
  ([p1, p2]) => {
    if (!p1) list1.value = 1
    if (!p2) list2.value = 1
  },
  { immediate: true },
)

const isCombatEsprit = computed(
  () => props.scenario.slug === COMBAT_ESPRIT_SLUG,
)

const isTournament = computed(() => Boolean(props.tournamentMatchId))

const canSubmit = computed(() => {
  if (!isTournament.value) return true
  return (
    (list1.value === 1 || list1.value === 2)
    && (list2.value === 1 || list2.value === 2)
    && (list1.value !== 2 || props.player1HasList2)
    && (list2.value !== 2 || props.player2HasList2)
  )
})

const submitLabel = computed(() => {
  if (isTournament.value) {
    return 'Soumettre le résultat (confirmation adverse)'
  }
  if (props.resolvedOutcome === 'player1_win') {
    return victoryLabel(props.player1.name)
  }
  if (props.resolvedOutcome === 'player2_win') {
    return victoryLabel(props.player2.name)
  }
  return 'Valider le match nul'
})

function victoryLabel(name: string) {
  const first = name.trim().charAt(0).toLowerCase()
  if ('aeiouhàâäéèêëïîôùûü'.includes(first)) {
    return `Valider la victoire d'${name}`
  }
  return `Valider la victoire de ${name}`
}

function clampObjectives(value: number) {
  return Math.min(10, Math.max(0, value))
}

function clampSurvivors(value: number) {
  return Math.min(300, Math.max(0, value))
}

function updateScore(field: keyof PartieScores, value: number) {
  emit('update:scores', { ...props.scores, [field]: value })
}

async function submit() {
  if (!isAuthenticated.value) {
    toast.error('Connectez-vous avec Discord pour enregistrer le résultat.')
    login()
    return
  }

  if (!canSubmit.value) {
    toast.error('Choisissez la liste de chaque joueur.')
    return
  }

  submitting.value = true
  try {
    if (props.tournamentMatchId) {
      await submitTournamentFromPartie(props.tournamentMatchId, {
        player1_objectives: clampObjectives(props.scores.player1Objectives),
        player1_survivors: clampSurvivors(props.scores.player1Survivors),
        player2_objectives: clampObjectives(props.scores.player2Objectives),
        player2_survivors: clampSurvivors(props.scores.player2Survivors),
        player1_list_slot: list1.value!,
        player2_list_slot: list2.value!,
      })
      toast.success('Résultat soumis — en attente de confirmation')
      emit('recorded')
      if (props.tournamentId) {
        router.push(`/tournoi/${props.tournamentId}`)
      } else {
        router.push('/tournois')
      }
      return
    }

    const record = await completeMatch(props.matchId, {
      outcome: props.resolvedOutcome,
      player1_objectives: clampObjectives(props.scores.player1Objectives),
      player1_survivors: clampSurvivors(props.scores.player1Survivors),
      player2_objectives: clampObjectives(props.scores.player2Objectives),
      player2_survivors: clampSurvivors(props.scores.player2Survivors),
    })
    if (record.counts_for_elo === false) {
      toast.success('Résultat enregistré')
    } else {
      toast.success(
        `${record.player1} ${Math.round(record.player1_old)} → ${Math.round(record.player1_new)} | ` +
          `${record.player2} ${Math.round(record.player2_old)} → ${Math.round(record.player2_new)}`,
      )
    }
    emit('recorded')
    router.push(`/matchs/${record.id}`)
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur inconnue')
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <div class="grid gap-6">
    <p class="page-description">
      Saisissez le résultat de la partie. Le vainqueur est déterminé par les points
      d'objectifs.
      <span v-if="isTournament">
        En tournoi, les listes d’armée sont obligatoires avant soumission.
      </span>
    </p>

    <div
      v-if="isCombatEsprit"
      class="rounded-lg border border-primary/30 bg-primary/5 px-4 py-3 text-sm text-muted-foreground"
    >
      Scénario spécial : la victoire peut aussi dépendre des objectifs secondaires
      terminés (3 pts chacun). Saisissez les points d'objectifs totaux ci-dessous.
    </div>

    <div class="neon-panel rounded-lg border border-primary/20 p-4 text-sm">
      <p>
        <span class="text-muted-foreground">Scénario :</span>
        <a
          v-if="scenario.url"
          :href="externalHref(scenario.url)"
          target="_blank"
          rel="noopener noreferrer"
          class="ml-1 font-medium text-primary hover:underline"
        >
          {{ scenario.name ?? scenario.other }}
        </a>
        <span v-else class="font-medium">{{ scenario.name ?? scenario.other }}</span>
      </p>
    </div>

    <div
      v-if="isTournament"
      class="grid grid-cols-1 gap-4 sm:grid-cols-2"
    >
      <div class="grid gap-2">
        <Label>{{ player1.name }} — {{ listLabel || 'Liste' }}</Label>
        <select
          v-if="player1HasList2"
          v-model.number="list1"
          class="h-9 rounded-md border border-input bg-transparent px-2 text-sm"
        >
          <option :value="undefined" disabled>Choisir…</option>
          <option :value="1">Liste 1</option>
          <option :value="2">Liste 2</option>
        </select>
        <span v-else class="flex h-9 items-center text-sm text-muted-foreground">
          Liste 1
        </span>
      </div>
      <div class="grid gap-2">
        <Label>{{ player2.name }} — {{ listLabel || 'Liste' }}</Label>
        <select
          v-if="player2HasList2"
          v-model.number="list2"
          class="h-9 rounded-md border border-input bg-transparent px-2 text-sm"
        >
          <option :value="undefined" disabled>Choisir…</option>
          <option :value="1">Liste 1</option>
          <option :value="2">Liste 2</option>
        </select>
        <span v-else class="flex h-9 items-center text-sm text-muted-foreground">
          Liste 1
        </span>
      </div>
    </div>

    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
      <section class="player-match-panel">
        <p class="player-match-panel-title">{{ player1.name }}</p>
        <div class="grid gap-3">
          <div class="grid gap-2">
            <Label :for="`p1-objectives`">Points d'objectifs (0–10)</Label>
            <Input
              :id="`p1-objectives`"
              :model-value="scores.player1Objectives"
              type="number"
              min="0"
              max="10"
              step="1"
              @update:model-value="
                updateScore('player1Objectives', Number($event) || 0)
              "
            />
          </div>
          <div class="grid gap-2">
            <Label :for="`p1-survivors`">Points de survivants (0–300)</Label>
            <Input
              :id="`p1-survivors`"
              :model-value="scores.player1Survivors"
              type="number"
              min="0"
              max="300"
              step="1"
              @update:model-value="
                updateScore('player1Survivors', Number($event) || 0)
              "
            />
          </div>
        </div>
      </section>

      <section class="player-match-panel">
        <p class="player-match-panel-title">{{ player2.name }}</p>
        <div class="grid gap-3">
          <div class="grid gap-2">
            <Label :for="`p2-objectives`">Points d'objectifs (0–10)</Label>
            <Input
              :id="`p2-objectives`"
              :model-value="scores.player2Objectives"
              type="number"
              min="0"
              max="10"
              step="1"
              @update:model-value="
                updateScore('player2Objectives', Number($event) || 0)
              "
            />
          </div>
          <div class="grid gap-2">
            <Label :for="`p2-survivors`">Points de survivants (0–300)</Label>
            <Input
              :id="`p2-survivors`"
              :model-value="scores.player2Survivors"
              type="number"
              min="0"
              max="300"
              step="1"
              @update:model-value="
                updateScore('player2Survivors', Number($event) || 0)
              "
            />
          </div>
        </div>
      </section>
    </div>

    <div class="flex flex-col gap-2 sm:flex-row sm:justify-between">
      <Button type="button" variant="outline" :disabled="submitting" @click="emit('back')">
        Précédent
      </Button>
      <Button type="button" :disabled="submitting || !canSubmit" @click="submit">
        <Swords class="size-4" />
        {{ submitting ? 'Enregistrement…' : submitLabel }}
      </Button>
    </div>
  </div>
</template>
