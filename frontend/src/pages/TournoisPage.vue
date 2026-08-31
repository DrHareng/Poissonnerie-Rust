<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Plus, Trophy } from '@lucide/vue'
import { toast } from 'vue-sonner'
import {
  createTournament,
  fetchPrefs,
  fetchTournaments,
  updatePrefs,
  type TournamentCompletedViewMode,
} from '@/lib/api'
import TournamentCompletedPodium from '@/components/TournamentCompletedPodium.vue'
import {
  formatRegistrationSummary,
  isTournamentCompleted,
  isTournamentRegistrationPhase,
  tournamentRegistrationCapacity,
} from '@/lib/tournamentDisplay'
import TournamentDescriptionWithRegistrants from '@/components/TournamentDescriptionWithRegistrants.vue'
import type { TournamentListEntry } from '@/types/elo'
import { useAuth } from '@/composables/useAuth'
import BracketTree from '@/components/BracketTree.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import TournamentPoolScenarioLinks from '@/components/TournamentPoolScenarioLinks.vue'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { tournoisTabs } from '@/lib/pageTitleTabs'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

const router = useRouter()
const route = useRoute()
const { isAdmin } = useAuth()

const tournaments = ref<TournamentListEntry[]>([])
const loading = ref(true)
const showCreate = ref(false)
const newName = ref('')
const newFormat = ref('round_of_16')
const creating = ref(false)
const completedViewMode = ref<TournamentCompletedViewMode>('compressed')

const isCompletedTab = computed(() => route.name === 'tournois-termines')
const isCompressedCompletedView = computed(
  () => isCompletedTab.value && completedViewMode.value === 'compressed',
)

const filteredTournaments = computed(() =>
  tournaments.value.filter((tournament) =>
    isCompletedTab.value
      ? isTournamentCompleted(tournament.status)
      : !isTournamentCompleted(tournament.status),
  ),
)

const listTitle = computed(() =>
  isCompletedTab.value ? 'Tournois terminés' : 'Tournois en cours',
)

const emptyMessage = computed(() =>
  isCompletedTab.value
    ? 'Aucun tournoi terminé pour l’instant.'
    : 'Aucun tournoi en cours pour l’instant.',
)

function viewToggleClass(mode: TournamentCompletedViewMode) {
  return completedViewMode.value === mode
    ? 'border-primary bg-primary! text-primary-foreground hover:bg-primary/90'
    : 'border-border bg-black text-white hover:text-primary'
}

function setCompletedViewMode(mode: TournamentCompletedViewMode) {
  if (completedViewMode.value === mode) return
  completedViewMode.value = mode
  void updatePrefs({ tournament_completed_view_mode: mode }).catch(() => {
    // Keep the local choice even if persistence fails.
  })
}

async function refresh() {
  loading.value = true
  try {
    tournaments.value = await fetchTournaments()
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur de chargement')
  } finally {
    loading.value = false
  }
}

async function create() {
  if (!newName.value.trim()) {
    toast.error('Indiquez un nom de tournoi.')
    return
  }
  creating.value = true
  try {
    const tournament = await createTournament({
      name: newName.value.trim(),
      bracket_format: newFormat.value,
    })
    toast.success(`Tournoi « ${tournament.name} » créé.`)
    showCreate.value = false
    newName.value = ''
    await refresh()
    router.push({ name: 'tournoi', params: { id: tournament.id } })
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur')
  } finally {
    creating.value = false
  }
}

onMounted(() => {
  void refresh()
  void fetchPrefs()
    .then((prefs) => {
      if (
        prefs.tournament_completed_view_mode === 'detailed'
        || prefs.tournament_completed_view_mode === 'compressed'
      ) {
        completedViewMode.value = prefs.tournament_completed_view_mode
      }
    })
    .catch(() => {
      // Keep the default view if prefs cannot be loaded.
    })
})
</script>

<template>
  <div class="page-stack">
    <PageTitleTabs
      :tabs="tournoisTabs"
      ariaLabel="Sections des tournois"
    />

    <Card v-if="isAdmin && showCreate && !isCompletedTab" class="neon-panel">
      <CardHeader>
        <CardTitle>Créer un tournoi</CardTitle>
      </CardHeader>
      <CardContent class="grid gap-4">
        <div class="grid gap-2">
          <Label for="tournament-name">Nom</Label>
          <Input id="tournament-name" v-model="newName" placeholder="Ex. Poissonnerie 2026" />
        </div>
        <div class="grid gap-2">
          <Label>Format d'arbre (4 poules)</Label>
          <Select v-model="newFormat">
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="quarters_direct">Quarts directs (1er + 2e)</SelectItem>
              <SelectItem value="round_of_16">Seizièmes (2e vs 3e, BYE 1ers)</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="flex gap-2">
          <Button :disabled="creating" @click="create">
            {{ creating ? 'Création...' : 'Créer' }}
          </Button>
          <Button variant="outline" @click="showCreate = false">Annuler</Button>
        </div>
      </CardContent>
    </Card>

    <Card class="neon-panel page-panel-scroll">
      <CardHeader>
        <CardTitle class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
          <div class="flex items-center gap-2">
            <Trophy class="size-5 text-primary" />
            {{ listTitle }}
          </div>
          <div class="flex flex-wrap items-center gap-2 self-start lg:self-auto">
            <template v-if="isCompletedTab">
              <span class="text-sm font-medium text-muted-foreground">Affichage :</span>
              <div class="flex items-center gap-0">
                <Button
                  type="button"
                  size="xs"
                  variant="outline"
                  class="rounded-r-none"
                  :class="viewToggleClass('detailed')"
                  @click="setCompletedViewMode('detailed')"
                >
                  Détaillé
                </Button>
                <Button
                  type="button"
                  size="xs"
                  variant="outline"
                  class="rounded-l-none border-l-0"
                  :class="viewToggleClass('compressed')"
                  @click="setCompletedViewMode('compressed')"
                >
                  Comprimé
                </Button>
              </div>
            </template>
            <Button
              v-if="isAdmin && !showCreate && !isCompletedTab"
              class="shrink-0"
              @click="showCreate = true"
            >
              <Plus class="size-4" />
              Nouveau tournoi
            </Button>
          </div>
        </CardTitle>
      </CardHeader>
      <CardContent class="lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
        <div
          v-if="loading"
          class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
        >
          Chargement...
        </div>
        <div
          v-else-if="filteredTournaments.length === 0"
          class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
        >
          {{ emptyMessage }}
        </div>
        <div
          v-else
          class="grid gap-3"
          :class="
            isCompressedCompletedView
              ? 'sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4'
              : undefined
          "
        >
          <button
            v-for="tournament in filteredTournaments"
            :key="tournament.id"
            type="button"
            class="grid gap-3 rounded-lg border p-4 text-left transition hover:border-primary/50 hover:bg-muted/30"
            @click="router.push({ name: 'tournoi', params: { id: tournament.id } })"
          >
            <div class="flex items-start justify-between gap-4">
              <div class="min-w-0 space-y-1">
                <p class="font-medium">{{ tournament.name }}</p>
                <p
                  v-if="!isCompressedCompletedView"
                  class="text-sm text-muted-foreground"
                >
                  {{
                    formatRegistrationSummary(
                      tournament.registered_count,
                      tournament.waitlist_count,
                      tournamentRegistrationCapacity(tournament.pool_count),
                    )
                  }}
                </p>
              </div>
              <Badge variant="outline" class="shrink-0">
                {{ tournament.display_status }}
              </Badge>
            </div>

            <TournamentCompletedPodium
              v-if="isCompressedCompletedView"
              :entries="tournament.top_four"
            />

            <template v-else>
              <TournamentDescriptionWithRegistrants
                v-if="
                  isTournamentRegistrationPhase(tournament.status)
                  && (tournament.description?.trim() || (tournament.registrations?.length ?? 0) > 0)
                "
                :description="tournament.description"
                :registrations="tournament.registrations ?? []"
              />
              <div
                v-else-if="tournament.description?.trim()"
                class="prose prose-sm max-w-none text-muted-foreground"
              >
                <MarkdownContent :source="tournament.description" />
              </div>
              <div
                v-if="(tournament.pool_scenarios?.length ?? 0) > 0"
                class="space-y-1"
              >
                <p class="text-xs font-medium text-muted-foreground">Scénarios de poules</p>
                <TournamentPoolScenarioLinks :scenarios="tournament.pool_scenarios ?? []" />
              </div>
              <BracketTree
                v-if="tournament.bracket_matches?.length"
                :matches="tournament.bracket_matches"
                compact
              />
            </template>
          </button>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
