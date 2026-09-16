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

export type PartieListSlots = {
  player1: number
  player2: number
}

const props = withDefaults(
  defineProps<{
    player1DisplayName: string
    player2DisplayName: string
    initial?: PartieLieutenant | null
    /** Mode tournoi : saisie des listes avant le jet. */
    tournamentMode?: boolean
    player1HasList2?: boolean
    player2HasList2?: boolean
    listLabel?: string
    initialList1?: number | null
    initialList2?: number | null
  }>(),
  {
    tournamentMode: false,
    player1HasList2: false,
    player2HasList2: false,
    listLabel: 'Liste',
    initialList1: null,
    initialList2: null,
  },
)

const emit = defineEmits<{
  back: []
  next: [value: PartieLieutenant, lists?: PartieListSlots]
}>()

const list1 = ref<number | undefined>(
  props.player1HasList2 ? (props.initialList1 ?? undefined) : 1,
)
const list2 = ref<number | undefined>(
  props.player2HasList2 ? (props.initialList2 ?? undefined) : 1,
)

const winner = ref<LieutenantWinner | undefined>(props.initial?.winner)
const winnerChoice = ref(props.initial?.winnerChoice)
const otherChoice = ref(props.initial?.otherChoice)

watch(
  () =>
    [
      props.player1HasList2,
      props.player2HasList2,
      props.initialList1,
      props.initialList2,
    ] as const,
  ([p1Has, p2Has, init1, init2]) => {
    list1.value = p1Has ? (init1 ?? list1.value ?? undefined) : 1
    list2.value = p2Has ? (init2 ?? list2.value ?? undefined) : 1
  },
)

const listsReady = computed(() => {
  if (!props.tournamentMode) return true
  return (
    (list1.value === 1 || list1.value === 2)
    && (list2.value === 1 || list2.value === 2)
    && (list1.value !== 2 || props.player1HasList2)
    && (list2.value !== 2 || props.player2HasList2)
  )
})

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

const showWinnerChoice = computed(
  () => listsReady.value && Boolean(winner.value),
)
const showOtherChoice = computed(
  () => listsReady.value && Boolean(winner.value && winnerChoice.value),
)

const canContinue = computed(
  () =>
    listsReady.value
    && Boolean(winner.value && winnerChoice.value && otherChoice.value),
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
  const lieutenant: PartieLieutenant = {
    winner: winner.value,
    winnerChoice: winnerChoice.value,
    otherChoice: otherChoice.value,
  }
  if (props.tournamentMode) {
    emit('next', lieutenant, {
      player1: list1.value!,
      player2: list2.value!,
    })
    return
  }
  emit('next', lieutenant)
}
</script>

<template>
  <div class="grid gap-6">
    <template v-if="tournamentMode">
      <p class="page-description">
        Choisissez d’abord la liste de chaque joueur, puis déterminez le jet de lieutenant.
      </p>
      <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div class="grid gap-2">
          <Label :for="'lieutenant-list-1'">
            {{ player1DisplayName }} — {{ listLabel }}
          </Label>
          <select
            v-if="player1HasList2"
            id="lieutenant-list-1"
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
          <Label :for="'lieutenant-list-2'">
            {{ player2DisplayName }} — {{ listLabel }}
          </Label>
          <select
            v-if="player2HasList2"
            id="lieutenant-list-2"
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
    </template>
    <p v-else class="page-description">
      Déterminez qui remporte le jet de lieutenant, puis répartissez l'initiative et
      le déploiement.
    </p>

    <div
      v-if="!tournamentMode || listsReady"
      class="grid gap-4"
    >
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
