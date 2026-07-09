<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { Plus, Medal } from '@lucide/vue'
import { toast } from 'vue-sonner'
import { createTournament, fetchTournaments } from '@/lib/api'
import { formatRegistrationSummary, topFourDisplayRows } from '@/lib/tournamentDisplay'
import type { TournamentListEntry } from '@/types/elo'
import { useAuth } from '@/composables/useAuth'
import PlayerLink from '@/components/PlayerLink.vue'
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

const router = useRouter()
const { isAdmin } = useAuth()

const tournaments = ref<TournamentListEntry[]>([])
const loading = ref(true)
const showCreate = ref(false)
const newName = ref('')
const newFormat = ref('quarters_direct')
const creating = ref(false)

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

onMounted(refresh)
</script>

<template>
  <div class="page-stack">
    <section class="page-header">
      <div class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div class="space-y-2">
          <h1 class="page-title">Tournois</h1>
          <p class="page-description">
            Phase de poules puis arbre éliminatoire.
          </p>
        </div>
        <Button v-if="isAdmin && !showCreate" @click="showCreate = true">
          <Plus class="size-4" />
          Nouveau tournoi
        </Button>
      </div>
    </section>

    <Card v-if="isAdmin && showCreate" class="neon-panel">
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

    <Card class="neon-panel">
      <CardHeader>
        <CardTitle class="flex items-center gap-2">
          <Medal class="size-5 text-primary" />
          Liste des tournois
        </CardTitle>
        <CardDescription>Cliquez pour voir le détail.</CardDescription>
      </CardHeader>
      <CardContent>
        <div
          v-if="loading"
          class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
        >
          Chargement...
        </div>
        <div
          v-else-if="tournaments.length === 0"
          class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
        >
          Aucun tournoi pour l'instant.
        </div>
        <div v-else class="grid gap-3">
          <button
            v-for="tournament in tournaments"
            :key="tournament.id"
            type="button"
            class="flex items-start justify-between gap-4 rounded-lg border p-4 text-left transition hover:border-primary/50 hover:bg-muted/30"
            @click="router.push({ name: 'tournoi', params: { id: tournament.id } })"
          >
            <div class="min-w-0 space-y-2">
              <p class="font-medium">{{ tournament.name }}</p>
              <p class="text-sm text-muted-foreground">
                {{ formatRegistrationSummary(tournament.approved_count, tournament.waitlist_count) }}
              </p>
              <ol
                v-if="tournament.status === 'completed' && tournament.top_four?.length"
                class="space-y-1 text-sm"
              >
                <li
                  v-for="row in topFourDisplayRows(tournament.top_four)"
                  :key="row.label"
                  class="flex items-center gap-2"
                >
                  <span class="w-8 shrink-0 tabular-nums text-muted-foreground">
                    {{ row.label }}
                  </span>
                  <span class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-0.5">
                    <template
                      v-for="(entry, index) in row.entries"
                      :key="entry.player_name"
                    >
                      <span
                        v-if="index > 0"
                        class="text-muted-foreground"
                      >
                        ·
                      </span>
                      <PlayerLink
                        :name="entry.player_name"
                        :display-name="entry.player_display_name"
                      />
                    </template>
                  </span>
                </li>
              </ol>
            </div>
            <Badge variant="outline" class="shrink-0">
              {{ tournament.display_status }}
            </Badge>
          </button>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
