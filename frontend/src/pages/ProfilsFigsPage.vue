<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import TtsFigurinesPanel from '@/components/TtsFigurinesPanel.vue'
import TtsProfilesPanel from '@/components/TtsProfilesPanel.vue'
import { profilsFigsTabs } from '@/lib/pageTitleTabs'

const route = useRoute()

const isProfilesTab = computed(() => route.name === 'profils-figs-profils')
const isFigsTab = computed(() => route.name === 'profils-figs-figs')
</script>

<template>
  <div class="page-stack">
    <PageTitleTabs
      ariaLabel="Profils et figurines"
      :tabs="profilsFigsTabs"
    />

    <TtsProfilesPanel v-if="isProfilesTab" />

    <TtsFigurinesPanel v-else-if="isFigsTab" />

    <p v-else class="text-sm text-muted-foreground">
      Listes TTS — section en cours de préparation.
    </p>
  </div>
</template>
