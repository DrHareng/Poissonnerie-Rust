<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'

const route = useRoute()
const title = computed(() => String(route.meta.title ?? 'Le Dauphiné'))
</script>

<template>
  <main class="dauphine-placeholder">
    <h1>{{ title }}</h1>
    <p>Section à venir.</p>
  </main>
</template>
