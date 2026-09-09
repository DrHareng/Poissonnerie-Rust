<script setup lang="ts">
import MarkdownContent from '@/components/MarkdownContent.vue'
import TournamentPoolScenarioLinks from '@/components/TournamentPoolScenarioLinks.vue'
import TournamentRegistrationList from '@/components/TournamentRegistrationList.vue'
import type { RegistrationSortInput } from '@/lib/tournamentDisplay'
import type { TournamentScenarioSlot } from '@/types/elo'

withDefaults(
  defineProps<{
    description?: string | null
    registrations: RegistrationSortInput[]
    showStatus?: boolean
    scenarios?: TournamentScenarioSlot[]
    compact?: boolean
    listMaxClass?: string
  }>(),
  {
    showStatus: false,
    scenarios: () => [],
    compact: false,
    listMaxClass: 'max-h-64',
  },
)
</script>

<template>
  <div class="grid gap-4 lg:grid-cols-4 lg:items-start">
    <div
      v-if="description?.trim() || (scenarios?.length ?? 0) > 0"
      class="grid min-w-0 gap-4 lg:col-span-3"
    >
      <div
        v-if="description?.trim()"
        class="prose prose-sm max-w-none text-muted-foreground"
      >
        <MarkdownContent :source="description" />
      </div>
      <div
        v-if="(scenarios?.length ?? 0) > 0"
        class="space-y-1"
      >
        <p
          :class="
            compact
              ? 'text-xs font-medium text-muted-foreground'
              : 'text-sm font-medium'
          "
        >
          Scénarios de poules
        </p>
        <TournamentPoolScenarioLinks :scenarios="scenarios ?? []" />
      </div>
    </div>
    <div
      class="flex min-w-0 flex-col gap-2"
      :class="
        description?.trim() || (scenarios?.length ?? 0) > 0
          ? 'lg:col-span-1'
          : 'lg:col-span-4'
      "
      @click.stop
    >
      <h3 class="text-base font-medium leading-snug">Inscrits</h3>
      <div
        class="min-h-0 overflow-y-auto pr-1"
        :class="listMaxClass"
      >
        <TournamentRegistrationList
          :registrations="registrations"
          :show-status="showStatus"
        />
      </div>
    </div>
  </div>
</template>
