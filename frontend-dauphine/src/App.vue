<script setup lang="ts">
import { computed } from 'vue'
import { RouterView, useRoute } from 'vue-router'
import { withBase } from '@/lib/basePath'

const route = useRoute()

const inProgress = computed(() => route.meta.inProgress === true)

const pageBackground = computed(() =>
  withBase(
    inProgress.value
      ? '/resources/fond_in_progress.png'
      : '/resources/fond.png',
  ),
)
</script>

<template>
  <div
    class="dauphine-shell"
    :class="{ 'dauphine-shell--in-progress': inProgress }"
    :style="{ '--dauphine-page-bg': `url('${pageBackground}')` }"
  >
    <RouterView />
  </div>
</template>
