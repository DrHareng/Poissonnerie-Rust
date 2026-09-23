<script setup lang="ts">
import TopBar from '@/components/TopBar.vue'
</script>

<template>
  <div class="dauphine-layout">
    <TopBar />
    <div class="dauphine-main">
      <RouterView />
    </div>
  </div>
</template>
