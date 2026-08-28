<script setup lang="ts">
import MarkdownContent from '@/components/MarkdownContent.vue'
import TournamentRegistrationList from '@/components/TournamentRegistrationList.vue'
import type { RegistrationSortInput } from '@/lib/tournamentDisplay'

defineProps<{
  description?: string | null
  registrations: RegistrationSortInput[]
  showStatus?: boolean
}>()
</script>

<template>
  <div class="grid gap-4 lg:grid-cols-4">
    <div
      v-if="description?.trim()"
      class="prose prose-sm max-w-none text-muted-foreground lg:col-span-3"
    >
      <MarkdownContent :source="description" />
    </div>
    <div
      class="min-w-0 space-y-2"
      :class="description?.trim() ? 'lg:col-span-1' : 'lg:col-span-4'"
    >
      <p class="text-xs font-medium text-muted-foreground">Inscrits</p>
      <TournamentRegistrationList
        :registrations="registrations"
        :show-status="showStatus"
      />
    </div>
  </div>
</template>
