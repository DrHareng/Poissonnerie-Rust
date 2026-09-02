<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { RouterView, useRoute } from 'vue-router'
import TopBar from '@/components/TopBar.vue'
import { Toaster } from '@/components/ui/sonner'
import { useAppSidePanelHost } from '@/composables/useAppSidePanel'
import { useBrandSideImageVisibility } from '@/composables/useBrandSideImageVisibility'
import { useSideImagePrefs } from '@/composables/useSideImagePrefs'

const route = useRoute()
const { customSideActive } = useAppSidePanelHost()
const { pickSideImage, enabledImages } = useSideImagePrefs()

const bodyRef = ref<HTMLElement | null>(null)
const sideImage = ref(pickSideImage())
const { showBrandImage, viewportTooSmall } = useBrandSideImageVisibility(
  sideImage,
  customSideActive,
  bodyRef,
)
const showSidePanel = computed(
  () => customSideActive.value || showBrandImage.value,
)

watch(
  () => route.path,
  () => {
    sideImage.value = pickSideImage()
  },
)

watch(enabledImages, (pool) => {
  if (!sideImage.value || !pool.some((image) => image.src === sideImage.value)) {
    sideImage.value = pickSideImage()
  }
})
</script>

<template>
  <div class="poissonnerie-shell">
    <div class="poissonnerie-backdrop" aria-hidden="true" />

    <div class="poissonnerie-inner">
      <TopBar />

      <div
        ref="bodyRef"
        class="poissonnerie-body"
        :class="{
          'poissonnerie-body--no-side': !showSidePanel,
          'poissonnerie-body--narrow-viewport': viewportTooSmall,
        }"
      >
        <aside
          class="poissonnerie-side-panel"
          :class="{
            'poissonnerie-side-panel--custom': customSideActive,
            'poissonnerie-side-panel--empty': !showSidePanel,
          }"
          :aria-hidden="customSideActive ? undefined : true"
        >
          <div
            id="app-side-panel"
            class="poissonnerie-side-slot"
            :class="{ hidden: !customSideActive }"
          />
          <img
            v-if="showBrandImage && sideImage"
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
