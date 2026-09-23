<script setup lang="ts">
import { RouterLink } from 'vue-router'
</script>

<template>
  <div class="mx-auto flex min-h-[50vh] max-w-lg flex-col items-center justify-center gap-4 px-6 py-16 text-center">
    <p class="font-display text-6xl tracking-wide text-primary/80">404</p>
    <h1 class="font-display text-2xl">Page introuvable</h1>
    <p class="text-sm text-muted-foreground">
      Cette adresse n’existe pas sur La Poissonnerie.
    </p>
    <!-- Emplacement prévu pour une illustration 404 -->
    <div
      class="mt-2 flex aspect-[16/9] w-full max-w-md items-center justify-center rounded-md border border-dashed border-border/60 bg-muted/30 text-xs text-muted-foreground"
      aria-hidden="true"
    >
      Illustration à venir
    </div>
    <RouterLink
      to="/"
      class="mt-2 text-sm text-primary underline-offset-4 hover:underline"
    >
      Retour à l’accueil
    </RouterLink>
  </div>
</template>
