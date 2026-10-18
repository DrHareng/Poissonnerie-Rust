<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import TtsFigurinesPanel from '@/components/TtsFigurinesPanel.vue'
import TtsProfilesPanel from '@/components/TtsProfilesPanel.vue'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { useAuth } from '@/composables/useAuth'
import { profilsFigsTabs } from '@/lib/pageTitleTabs'

const route = useRoute()
const { isAdmin, loading: authLoading, initialized } = useAuth()

const isProfilesTab = computed(() => route.name === 'profils-figs-profils')
const isFigsTab = computed(() => route.name === 'profils-figs-figs')
</script>

<template>
  <div class="page-stack">
    <PageTitleTabs
      v-if="isAdmin"
      ariaLabel="Profils et figurines"
      :tabs="profilsFigsTabs"
    />

    <div v-if="authLoading || !initialized" class="text-sm text-muted-foreground">
      Chargement...
    </div>

    <Alert v-else-if="!isAdmin" variant="destructive" class="neon-panel-accent">
      <AlertTitle>Accès réservé</AlertTitle>
      <AlertDescription>
        Cette page n’est disponible que pour les administrateurs.
      </AlertDescription>
    </Alert>

    <TtsProfilesPanel v-else-if="isProfilesTab" />

    <TtsFigurinesPanel v-else-if="isFigsTab" />

    <p v-else class="text-sm text-muted-foreground">
      Listes TTS — section en cours de préparation.
    </p>
  </div>
</template>
