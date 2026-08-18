<script setup lang="ts">
import { ref, watch } from 'vue'
import { RouterView, useRoute } from 'vue-router'
import TopBar from '@/components/TopBar.vue'
import { Toaster } from '@/components/ui/sonner'
import { useAppSidePanelHost } from '@/composables/useAppSidePanel'

const SIDE_IMAGES = ['/brand/side_01.png', '/brand/side_02.png', '/brand/side_03.png', '/brand/side_04.png'] as const

function pickSideImage() {
  return SIDE_IMAGES[Math.floor(Math.random() * SIDE_IMAGES.length)]!
}

const route = useRoute()
const sideImage = ref(pickSideImage())
const { customSideActive } = useAppSidePanelHost()

watch(
  () => route.path,
  () => {
    sideImage.value = pickSideImage()
  },
)
</script>

<template>
  <div class="poissonnerie-shell">
    <div class="poissonnerie-backdrop" aria-hidden="true" />

    <div class="poissonnerie-inner">
      <TopBar />

      <div class="poissonnerie-body">
        <aside
          class="poissonnerie-side-panel"
          :class="{ 'poissonnerie-side-panel--custom': customSideActive }"
          :aria-hidden="customSideActive ? undefined : true"
        >
          <div
            id="app-side-panel"
            class="poissonnerie-side-slot"
            :class="{ hidden: !customSideActive }"
          />
          <img
            v-show="!customSideActive"
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

    <Toaster theme="dark" rich-colors position="top-center" class="toaster-overlay" />
  </div>
</template>
