<script setup lang="ts">
import { withBase } from '@/lib/basePath'
import AppLayout from '@/layouts/AppLayout.vue'

const pageBackground = withBase('/resources/fond.png')
</script>

<template>
  <div
    class="dauphine-shell"
    :style="{ '--dauphine-page-bg': `url('${pageBackground}')` }"
  >
    <AppLayout />
  </div>
</template>
