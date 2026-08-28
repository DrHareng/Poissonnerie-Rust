<script setup lang="ts">
import { computed } from 'vue'
import PlayerLink from '@/components/PlayerLink.vue'
import {
  registrationListItemClass,
  registrationStatusLabel,
  sortRegistrationsForDisplay,
  type RegistrationSortInput,
} from '@/lib/tournamentDisplay'

const props = withDefaults(
  defineProps<{
    registrations: RegistrationSortInput[]
    showStatus?: boolean
  }>(),
  {
    showStatus: false,
  },
)

const sortedRegistrations = computed(() =>
  sortRegistrationsForDisplay(props.registrations),
)
</script>

<template>
  <ul v-if="sortedRegistrations.length > 0" class="grid gap-1.5">
    <li
      v-for="reg in sortedRegistrations"
      :key="reg.player_name"
      class="min-w-0"
      :class="registrationListItemClass(reg)"
    >
      <PlayerLink
        :name="reg.player_name"
        :display-name="reg.player_display_name"
        class="text-sm"
      />
      <p
        v-if="showStatus"
        class="text-xs text-muted-foreground"
      >
        {{ registrationStatusLabel(reg) }}
      </p>
    </li>
  </ul>
  <p v-else class="text-sm text-muted-foreground italic">
    Aucun inscrit pour l'instant.
  </p>
</template>
