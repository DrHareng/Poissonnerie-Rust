<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { Swords } from '@lucide/vue'
import { COMBAT_ESPRIT_SLUG } from '@/lib/combatEspritDraft'
import { recordMatch } from '@/lib/api'
import type { PartiePlayerSlot, PartieScenario, PartieScores } from '@/composables/usePartieFlow'
import type { MatchOutcome } from '@/types/elo'
import { useAuth } from '@/composables/useAuth'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

const props = defineProps<{
  player1: PartiePlayerSlot
  player2: PartiePlayerSlot
  scenario: PartieScenario
  scores: PartieScores
  resolvedOutcome: MatchOutcome
  scenarioPayload: () =>
    | { scenario_id: number }
    | { scenario_other: string }
    | undefined
}>()

const emit = defineEmits<{
  back: []
  'update:scores': [scores: PartieScores]
  recorded: []
}>()

const router = useRouter()
const { isAuthenticated, login } = useAuth()
const submitting = ref(false)

const isCombatEsprit = computed(
  () => props.scenario.slug === COMBAT_ESPRIT_SLUG,
)

const submitLabel = computed(() => {
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

  submitting.value = true
  try {
    const record = await recordMatch(
      props.player1.name,
      props.player2.name,
      props.resolvedOutcome,
      {
        player1_objectives: clampObjectives(props.scores.player1Objectives),
        player1_survivors: clampSurvivors(props.scores.player1Survivors),
        player2_objectives: clampObjectives(props.scores.player2Objectives),
        player2_survivors: clampSurvivors(props.scores.player2Survivors),
      },
      {
        player1_army_id: props.player1.armyId,
        player2_army_id: props.player2.armyId,
      },
      props.scenarioPayload(),
    )
    toast.success(
      `${record.player1} ${Math.round(record.player1_old)} → ${Math.round(record.player1_new)} | ` +
        `${record.player2} ${Math.round(record.player2_old)} → ${Math.round(record.player2_new)}`,
    )
    emit('recorded')
    router.push('/matchs')
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
        <span class="font-medium">{{ scenario.name ?? scenario.other }}</span>
      </p>
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
      <Button type="button" :disabled="submitting" @click="submit">
        <Swords class="size-4" />
        {{ submitting ? 'Enregistrement…' : submitLabel }}
      </Button>
    </div>
  </div>
</template>
