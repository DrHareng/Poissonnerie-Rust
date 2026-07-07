<script setup lang="ts">
import { ref, watch } from 'vue'
import { RouterView, useRoute } from 'vue-router'
import TopBar from '@/components/TopBar.vue'
import { Toaster } from '@/components/ui/sonner'

const SIDE_IMAGES = ['/brand/side_01.png', '/brand/side_02.png', '/brand/side_03.png'] as const

function pickSideImage() {
  return SIDE_IMAGES[Math.floor(Math.random() * SIDE_IMAGES.length)]!
}

const route = useRoute()
const sideImage = ref(pickSideImage())

watch(() => route.path, () => {
  sideImage.value = pickSideImage()
})
</script>

<template>
  <div class="poissonnerie-shell">
    <div class="poissonnerie-backdrop" aria-hidden="true" />

    <Toaster rich-colors position="top-right" />

    <div class="poissonnerie-inner">
      <TopBar />

      <div class="poissonnerie-body">
        <aside class="poissonnerie-side-panel" aria-hidden="true">
          <img
            :src="sideImage"
            alt=""
            class="poissonnerie-side-image"
          />
        </aside>

        <main class="poissonnerie-content">
          <RouterView />
        </main>
      </div>
    </div>
  </div>
</template>
