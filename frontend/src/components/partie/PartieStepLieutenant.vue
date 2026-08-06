<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { PartieLieutenant, LieutenantWinner } from '@/lib/lieutenantRoll'
import {
  choiceLabel,
  otherPlayerChoices,
  WINNER_CHOICES,
} from '@/lib/lieutenantRoll'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

const props = defineProps<{
  player1DisplayName: string
  player2DisplayName: string
  initial?: PartieLieutenant | null
}>()

const emit = defineEmits<{
  back: []
  next: [value: PartieLieutenant]
}>()

const winner = ref<LieutenantWinner | undefined>(props.initial?.winner)
const winnerChoice = ref(props.initial?.winnerChoice)
const otherChoice = ref(props.initial?.otherChoice)

const winnerDisplayName = computed(() =>
  winner.value === 'player1'
    ? props.player1DisplayName
    : winner.value === 'player2'
      ? props.player2DisplayName
      : '',
)

const otherDisplayName = computed(() =>
  winner.value === 'player1'
    ? props.player2DisplayName
    : winner.value === 'player2'
      ? props.player1DisplayName
      : '',
)

const otherChoices = computed(() =>
  winnerChoice.value ? otherPlayerChoices(winnerChoice.value) : [],
)

const showWinnerChoice = computed(() => Boolean(winner.value))
const showOtherChoice = computed(
  () => Boolean(winner.value && winnerChoice.value),
)

const canContinue = computed(
  () => Boolean(winner.value && winnerChoice.value && otherChoice.value),
)

watch(winner, () => {
  winnerChoice.value = undefined
  otherChoice.value = undefined
})

watch(winnerChoice, () => {
  otherChoice.value = undefined
})

function submit() {
  if (!canContinue.value || !winner.value || !winnerChoice.value || !otherChoice.value) {
    return
  }
  emit('next', {
    winner: winner.value,
    winnerChoice: winnerChoice.value,
    otherChoice: otherChoice.value,
  })
}
</script>

<template>
  <div class="grid gap-6">
    <p class="page-description">
      Déterminez qui remporte le jet de lieutenant, puis répartissez l'initiative et
      le déploiement.
    </p>

    <div class="grid gap-4">
      <div class="grid gap-2">
        <Label for="lieutenant-winner">Jet de lieutenant</Label>
        <div class="flex flex-col gap-2 sm:flex-row sm:items-center">
          <Select v-model="winner">
            <SelectTrigger id="lieutenant-winner" class="w-full sm:max-w-xs">
              <SelectValue placeholder="Choisir le vainqueur" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="player1">
                {{ player1DisplayName }}
              </SelectItem>
              <SelectItem value="player2">
                {{ player2DisplayName }}
              </SelectItem>
            </SelectContent>
          </Select>
          <span class="text-sm text-muted-foreground">a gagné le jet de lieutenant</span>
        </div>
      </div>

      <div v-if="showWinnerChoice" class="grid gap-2">
        <div class="flex flex-col gap-2 sm:flex-row sm:items-center">
          <span class="shrink-0 text-sm font-medium text-primary">
            {{ winnerDisplayName }}
          </span>
          <Select v-model="winnerChoice">
            <SelectTrigger
              :id="`lieutenant-winner-choice-${winner}`"
              class="w-full sm:max-w-md"
            >
              <SelectValue placeholder="Choisir une option" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="choice in WINNER_CHOICES"
                :key="choice.value"
                :value="choice.value"
              >
                {{ choice.label }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      <div v-if="showOtherChoice" class="grid gap-2">
        <div class="flex flex-col gap-2 sm:flex-row sm:items-center">
          <span class="shrink-0 text-sm font-medium text-primary">
            {{ otherDisplayName }}
          </span>
          <Select v-model="otherChoice">
            <SelectTrigger
              :id="`lieutenant-other-choice-${winner}`"
              class="w-full sm:max-w-md"
            >
              <SelectValue placeholder="Choisir une option" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="choice in otherChoices"
                :key="choice.value"
                :value="choice.value"
              >
                {{ choice.label }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
    </div>

    <div
      v-if="winner && winnerChoice && otherChoice"
      class="rounded-lg border border-primary/25 bg-primary/5 px-4 py-3 text-sm"
    >
      <p>
        <span class="font-medium text-primary">{{ winnerDisplayName }}</span>
        {{ choiceLabel(winnerChoice) }}
      </p>
      <p class="mt-1">
        <span class="font-medium text-primary">{{ otherDisplayName }}</span>
        {{ choiceLabel(otherChoice) }}
      </p>
    </div>

    <div class="flex flex-col gap-2 sm:flex-row sm:justify-between">
      <Button type="button" variant="outline" @click="emit('back')">
        Précédent
      </Button>
      <Button type="button" :disabled="!canContinue" @click="submit">
        Valider
      </Button>
    </div>
  </div>
</template>
