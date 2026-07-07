<script setup lang="ts">
import { History } from '@lucide/vue'
import type { MatchRecord } from '@/types/elo'
import MatchResultBadges from '@/components/MatchResultBadges.vue'
import ArmyLogo from '@/components/ArmyLogo.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'

defineProps<{
  matches: MatchRecord[]
  loading?: boolean
}>()

function formatDate(timestamp: number) {
  return new Intl.DateTimeFormat('fr-FR', {
    dateStyle: 'short',
    timeStyle: 'short',
  }).format(new Date(timestamp * 1000))
}
</script>

<template>
  <Card class="neon-panel page-panel-scroll">
    <CardHeader class="lg:shrink-0">
      <CardTitle class="flex items-center gap-2">
        <History class="size-5 text-primary" />
        Derniers matchs
      </CardTitle>
      <CardDescription>
        Les 20 matchs les plus récents, du plus récent au plus ancien.
      </CardDescription>
    </CardHeader>
    <CardContent class="lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
      <div
        v-if="loading"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        Chargement des matchs...
      </div>

      <div
        v-else-if="matches.length === 0"
        class="rounded-lg border border-dashed p-8 text-center text-muted-foreground"
      >
        Aucun match enregistré pour l'instant.
      </div>

      <Table v-else>
        <TableHeader class="sticky top-0 z-10 bg-card/95 backdrop-blur">
          <TableRow>
            <TableHead>Date</TableHead>
            <TableHead class="text-right">Joueur 1</TableHead>
            <TableHead class="w-10" aria-hidden="true" />
            <TableHead>Scénario</TableHead>
            <TableHead class="text-center">Résultat</TableHead>
            <TableHead class="w-10" aria-hidden="true" />
            <TableHead>Joueur 2</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="match in matches" :key="match.id">
            <TableCell class="whitespace-nowrap text-muted-foreground">
              {{ formatDate(match.recorded_at) }}
            </TableCell>
            <TableCell class="text-right">
              <PlayerLink
                :name="match.player1"
                :display-name="match.player1_display_name"
              />
            </TableCell>
            <TableCell class="w-10 px-2">
              <ArmyLogo :army-id="match.player1_army_id" />
            </TableCell>
            <TableCell />
            <TableCell>
              <MatchResultBadges :match="match" />
            </TableCell>
            <TableCell class="w-10 px-2">
              <ArmyLogo :army-id="match.player2_army_id" />
            </TableCell>
            <TableCell>
              <PlayerLink
                :name="match.player2"
                :display-name="match.player2_display_name"
              />
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </CardContent>
  </Card>
</template>
