<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import {
  completeTournamentRegistrationLists,
  startTournamentPartie,
} from '@/lib/api'
import { normalizeArmyListCode } from '@/lib/armyList'
import { COUPE_REQUIRES_NETWORK } from '@/lib/partieOffline'
import { phaseLabels } from '@/lib/tournamentPhase'
import {
  registrationListsFullyValidated,
  registrationStatusLabel,
} from '@/lib/tournamentDisplay'
import { useAuth } from '@/composables/useAuth'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import ArmyListQuickActions from '@/components/ArmyListQuickActions.vue'
import TournamentMatchCard from '@/components/TournamentMatchCard.vue'
import type { TournamentMatchForm } from '@/components/TournamentMatchCard.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import type { TournamentListEntry, TournamentMatch } from '@/types/elo'

const props = defineProps<{
  tournament: TournamentListEntry
}>()

const emit = defineEmits<{
  refreshed: []
}>()

const router = useRouter()
const { player } = useAuth()
const { showAdminUi: isAdmin } = useAdminEditMode()
const { isOnline } = useNetworkStatus()

const registration = computed(() => props.tournament.my_registration ?? null)
const upcoming = computed(() => props.tournament.my_upcoming_matches ?? [])

const listsValidated = computed(() =>
  registrationListsFullyValidated(registration.value),
)

const needsLists = computed(
  () =>
    !!registration.value
    && !registration.value.has_army_lists
    && (registration.value.status === 'pending'
      || registration.value.status === 'waitlisted'
      || registration.value.status === 'approved'),
)

const canEditLists = computed(() => {
  const status = props.tournament.status
  return (
    !!registration.value
    && (status === 'registration_open'
      || status === 'registration_closed'
      || status === 'draft'
      || status === 'started')
  )
})

/** Formulaire listes tant qu’on peut encore les saisir/modifier et qu’on n’est pas en mode matchs-only. */
const showEditableLists = computed(() => {
  if (!canEditLists.value || !registration.value) return false
  if (
    (props.tournament.status === 'started' || props.tournament.status === 'completed')
    && listsValidated.value
  ) {
    return false
  }
  return true
})

const showReadonlyLists = computed(
  () =>
    !!registration.value?.has_army_lists
    && (props.tournament.status === 'started' || props.tournament.status === 'completed')
    && listsValidated.value,
)

const list1 = ref('')
const list2 = ref('')
const savingLists = ref(false)

watch(
  () =>
    [
      registration.value?.army_list_1 ?? null,
      registration.value?.army_list_2 ?? null,
      registration.value?.has_army_lists ?? false,
    ] as const,
  ([code1, code2, hasLists]) => {
    if (code1) {
      list1.value = normalizeArmyListCode(code1)
      list2.value = code2 ? normalizeArmyListCode(code2) : ''
    } else if (!hasLists) {
      list1.value = ''
      list2.value = ''
    }
  },
  { immediate: true },
)

const matchForms = reactive<Record<number, TournamentMatchForm>>({})

function getForm(match: TournamentMatch): TournamentMatchForm {
  if (!matchForms[match.id]) {
    matchForms[match.id] = {
      p1: match.player1_objectives,
      p2: match.player2_objectives,
      s1: match.player1_survivors,
      s2: match.player2_survivors,
    }
  }
  return matchForms[match.id]
}

function matchStatusLabel(match: TournamentMatch) {
  if (match.is_unplayed) return 'Non joué'
  if (match.is_forfeit && match.status === 'submitted') return 'Forfait à confirmer'
  if (match.is_forfeit) return 'Forfait'
  if (match.status === 'scheduled' && match.elo_match_id) return 'En cours'
  if (match.status === 'scheduled') return 'À jouer'
  if (match.status === 'submitted') return 'À confirmer'
  if (match.status === 'confirmed') return 'Terminé'
  return match.status
}

function canInteract(match: TournamentMatch) {
  if (isAdmin.value) return true
  const me = player.value?.name?.toLowerCase()
  if (!me) return false
  return (
    match.player1?.toLowerCase() === me
    || match.player2?.toLowerCase() === me
  )
}

async function saveLists() {
  const code1 = normalizeArmyListCode(list1.value)
  const code2 = normalizeArmyListCode(list2.value)
  if (!code1) {
    toast.error('Indiquez le code de la liste 1.')
    return
  }
  if (code2 && code2 === code1) {
    toast.error('Les deux listes doivent être différentes.')
    return
  }
  savingLists.value = true
  try {
    await completeTournamentRegistrationLists(props.tournament.id, code1, code2)
    toast.success(
      needsLists.value
        ? 'Listes enregistrées — inscription en attente de validation'
        : 'Listes mises à jour',
    )
    emit('refreshed')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur')
  } finally {
    savingLists.value = false
  }
}

async function startPartie(match: TournamentMatch) {
  if (!isOnline.value) {
    toast.error(COUPE_REQUIRES_NETWORK)
    return
  }
  try {
    const record = await startTournamentPartie(match.id)
    emit('refreshed')
    await router.push({
      name: 'partie-resume',
      params: { id: String(record.id) },
      query: { tournamentMatchId: String(match.id) },
    })
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible de démarrer la partie')
  }
}

async function resumePartie(match: TournamentMatch) {
  if (!isOnline.value) {
    toast.error(COUPE_REQUIRES_NETWORK)
    return
  }
  if (!match.elo_match_id) {
    await startPartie(match)
    return
  }
  await router.push({
    name: 'partie-resume',
    params: { id: String(match.elo_match_id) },
    query: { tournamentMatchId: String(match.id) },
  })
}

function openTournament() {
  void router.push({ name: 'tournoi', params: { id: props.tournament.id } })
}
</script>

<template>
  <Card class="neon-panel shrink-0">
    <CardHeader class="pb-3">
      <button
        type="button"
        class="group flex w-full items-start justify-between gap-2 text-left"
        @click="openTournament"
      >
        <div class="min-w-0">
          <CardTitle class="text-base group-hover:text-primary">
            {{ tournament.name }}
          </CardTitle>
          <CardDescription class="mt-1">
            {{ tournament.display_status }}
          </CardDescription>
        </div>
        <Badge
          v-if="registration"
          variant="outline"
          class="shrink-0"
        >
          {{ registrationStatusLabel(registration) }}
        </Badge>
      </button>
    </CardHeader>
    <CardContent class="grid gap-3">
      <template v-if="showEditableLists && registration">
        <div class="grid gap-2">
          <Label :for="`my-t-${tournament.id}-list-1`">Liste 1</Label>
          <div class="flex flex-wrap items-center gap-2">
            <Input
              :id="`my-t-${tournament.id}-list-1`"
              v-model="list1"
              class="min-w-0 flex-1 text-xs"
              placeholder="Code Army…"
            />
            <ArmyListQuickActions :code="list1" />
          </div>
        </div>
        <div class="grid gap-2">
          <Label :for="`my-t-${tournament.id}-list-2`">Liste 2 (optionnel)</Label>
          <div class="flex flex-wrap items-center gap-2">
            <Input
              :id="`my-t-${tournament.id}-list-2`"
              v-model="list2"
              class="min-w-0 flex-1 text-xs"
              placeholder="Code Army… (optionnel)"
            />
            <ArmyListQuickActions :code="list2" />
          </div>
        </div>
        <Button
          size="sm"
          class="w-fit"
          :disabled="savingLists"
          @click="saveLists"
        >
          {{
            savingLists
              ? 'Enregistrement…'
              : needsLists
                ? "Valider l'inscription"
                : 'Mettre à jour les listes'
          }}
        </Button>
      </template>

      <template v-else-if="showReadonlyLists && registration">
        <div class="grid gap-2">
          <div class="flex flex-wrap items-center gap-2">
            <span class="w-14 shrink-0 text-xs text-muted-foreground">Liste 1</span>
            <Input
              :model-value="registration.army_list_1 ?? ''"
              readonly
              class="min-w-0 flex-1 text-xs"
            />
            <ArmyListQuickActions :code="registration.army_list_1" />
          </div>
          <div
            v-if="registration.army_list_2"
            class="flex flex-wrap items-center gap-2"
          >
            <span class="w-14 shrink-0 text-xs text-muted-foreground">Liste 2</span>
            <Input
              :model-value="registration.army_list_2"
              readonly
              class="min-w-0 flex-1 text-xs"
            />
            <ArmyListQuickActions :code="registration.army_list_2" />
          </div>
          <p
            v-else
            class="text-xs text-muted-foreground italic"
          >
            pas de liste 2
          </p>
        </div>
      </template>

      <div
        v-if="upcoming.length > 0"
        class="grid gap-2"
      >
        <h3 class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
          À venir
        </h3>
        <TournamentMatchCard
          v-for="match in upcoming"
          :key="match.id"
          compact
          opponent-focused
          :match="match"
          :form="getForm(match)"
          :can-interact="canInteract(match)"
          :is-admin="isAdmin"
          :current-player-name="player?.name"
          :player1-army-id="match.player1_army_id ?? undefined"
          :player2-army-id="match.player2_army_id ?? undefined"
          :status-label="matchStatusLabel(match)"
          :phase-label="phaseLabels[match.phase] ?? match.phase"
          :is-online="isOnline"
          @start="startPartie(match)"
          @resume="resumePartie(match)"
        />
      </div>
    </CardContent>
  </Card>
</template>
