<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { Army, RankedPlayer } from '@/types/elo'
import PlayerPicker from '@/components/PlayerPicker.vue'
import SectorialPicker from '@/components/SectorialPicker.vue'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'

const props = defineProps<{
  players: RankedPlayer[]
  armies: Army[]
  loading?: boolean
  armiesLoading?: boolean
  lockedPlayer1Name?: string
  initialPlayer1?: string
  initialPlayer2?: string
  initialArmy1?: number
  initialArmy2?: number
}>()

const emit = defineEmits<{
  back: []
  next: [payload: { player1: string; army1: number; player2: string; army2: number }]
}>()

const player1 = ref<string | undefined>(props.lockedPlayer1Name ?? props.initialPlayer1)
const player2 = ref<string | undefined>(props.initialPlayer2)
const army1 = ref<string | undefined>(
  props.initialArmy1 != null ? String(props.initialArmy1) : undefined,
)
const army2 = ref<string | undefined>(
  props.initialArmy2 != null ? String(props.initialArmy2) : undefined,
)

const playerOptions = computed(() =>
  props.players.map((player) => ({
    label: `${player.display_name} (${Math.round(player.rating)})`,
    value: player.name,
  })),
)

const player1Options = computed(() =>
  playerOptions.value.filter((option) => option.value !== player2.value),
)

const player2Options = computed(() =>
  playerOptions.value.filter((option) => option.value !== player1.value),
)

const canContinue = computed(
  () =>
    Boolean(player1.value && player2.value && army1.value && army2.value) &&
    player1.value !== player2.value,
)

const lockedPlayer1 = computed(() =>
  props.players.find((player) => player.name === props.lockedPlayer1Name) ?? null,
)

watch(
  () => props.lockedPlayer1Name,
  (value) => {
    player1.value = value ?? undefined
  },
  { immediate: true },
)

watch(player1, () => {
  army1.value = undefined
})

watch(player2, () => {
  army2.value = undefined
})

function submit() {
  if (!canContinue.value || !player1.value || !player2.value || !army1.value || !army2.value) {
    return
  }
  emit('next', {
    player1: player1.value,
    army1: Number(army1.value),
    player2: player2.value,
    army2: Number(army2.value),
  })
}
</script>

<template>
  <div class="grid gap-6">
    <p class="page-description">
      Sélectionnez les deux joueurs et leurs sectorielles pour cette partie.
    </p>

    <Alert v-if="!lockedPlayer1Name" variant="destructive">
      <AlertTitle>Joueur 1 requis</AlertTitle>
      <AlertDescription>
        Démarrer une partie exige un compte Discord lié à un joueur Poissonnerie.
      </AlertDescription>
    </Alert>

    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
      <section class="player-match-panel">
        <p class="player-match-panel-title">Joueur 1</p>
        <div class="grid gap-2">
          <Label>Joueur</Label>
          <div
            class="flex min-h-10 items-center rounded-md border border-primary/25 bg-muted/30 px-3 text-sm"
          >
            {{ lockedPlayer1?.display_name ?? lockedPlayer1Name ?? 'Connexion requise' }}
          </div>
        </div>
        <div class="grid gap-2">
          <Label>Sectorielle</Label>
          <SectorialPicker
            v-model="army1"
            :armies="armies"
            :disabled="!lockedPlayer1Name || !player1 || armiesLoading || armies.length === 0"
            :placeholder="
              player1
                ? 'Tapez pour chercher une sectorielle'
                : 'Sélectionnez d\'abord le joueur'
            "
          />
        </div>
      </section>

      <section class="player-match-panel">
        <p class="player-match-panel-title">Joueur 2</p>
        <div class="grid gap-2">
          <Label>Joueur</Label>
          <PlayerPicker
            v-model="player2"
            :options="player2Options"
            :disabled="loading || playerOptions.length < 2"
            placeholder="Tapez pour chercher un joueur"
          />
        </div>
        <div class="grid gap-2">
          <Label>Sectorielle</Label>
          <SectorialPicker
            v-model="army2"
            :armies="armies"
            :disabled="!player2 || armiesLoading || armies.length === 0"
            :placeholder="
              player2
                ? 'Tapez pour chercher une sectorielle'
                : 'Sélectionnez d\'abord le joueur'
            "
          />
        </div>
      </section>
    </div>

    <div class="flex flex-col gap-2 sm:flex-row sm:justify-end">
      <Button type="button" :disabled="!lockedPlayer1Name || !canContinue" @click="submit">
        Valider
      </Button>
    </div>
  </div>
</template>
