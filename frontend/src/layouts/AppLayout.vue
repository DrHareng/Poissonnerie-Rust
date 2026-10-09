<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { RouterView, useRoute } from 'vue-router'
import { X } from '@lucide/vue'
import BottomBar from '@/components/BottomBar.vue'
import TopBar from '@/components/TopBar.vue'
import { Toaster } from '@/components/ui/sonner'
import { Button } from '@/components/ui/button'
import { useAppSidePanelHost } from '@/composables/useAppSidePanel'
import { useBrandSideImageVisibility } from '@/composables/useBrandSideImageVisibility'
import { useSideImagePrefs } from '@/composables/useSideImagePrefs'

const route = useRoute()
const { customSideActive } = useAppSidePanelHost()
const {
  pickSideImage,
  enabledImages,
  forcedSideImageSrc,
  sideImageDismiss,
} = useSideImagePrefs()

const bodyRef = ref<HTMLElement | null>(null)
const randomSideImage = ref(pickSideImage())
const sideImage = computed(() => forcedSideImageSrc.value ?? randomSideImage.value)
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
    randomSideImage.value = pickSideImage()
  },
)

watch(enabledImages, (pool) => {
  if (forcedSideImageSrc.value) return
  if (!randomSideImage.value || !pool.some((image) => image.src === randomSideImage.value)) {
    randomSideImage.value = pickSideImage()
  }
})

watch(forcedSideImageSrc, (forced) => {
  if (!forced && !randomSideImage.value) {
    randomSideImage.value = pickSideImage()
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
          :aria-hidden="customSideActive || sideImageDismiss ? undefined : true"
        >
          <div
            id="app-side-panel"
            class="poissonnerie-side-slot"
            :class="{ hidden: !customSideActive }"
          />
          <div
            v-if="showBrandImage && sideImage"
            class="relative h-full min-h-0 w-full"
          >
            <img
              :src="sideImage"
              alt=""
              class="poissonnerie-side-image"
            />
            <Button
              v-if="sideImageDismiss"
              type="button"
              size="icon"
              variant="secondary"
              class="absolute top-2 right-2 z-10 border border-border shadow-lg"
              title="Revenir à la modification du profil"
              aria-label="Revenir à la modification du profil"
              @click="sideImageDismiss()"
            >
              <X class="size-5" />
            </Button>
          </div>
        </aside>

        <main class="poissonnerie-content">
          <RouterView />
        </main>
      </div>

      <BottomBar />
    </div>

    <Toaster theme="dark" rich-colors position="top-center" class="toaster-overlay" />
  </div>
</template>
