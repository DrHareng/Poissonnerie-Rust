import { computed, ref, watch } from 'vue'
import { SIDE_IMAGES, isSideImageId, type SideImageId } from '@/lib/sideImages'

const STORAGE_KEY = 'poissonnerie-side-images-disabled'

function readStored(): SideImageId[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed.filter(isSideImageId)
  } catch {
    return []
  }
}

const disabledIds = ref<SideImageId[]>(readStored())

/** Image imposée (ex. image de profil d'un joueur consulté). */
const forcedSideImageSrc = ref<string | null>(null)

/** Action de fermeture affichée en overlay sur l'image latérale (ex. fin de prévisualisation). */
const sideImageDismiss = ref<(() => void) | null>(null)

watch(disabledIds, (value) => {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(value))
  } catch {
    /* ignore */
  }
})

const enabledImages = computed(() =>
  SIDE_IMAGES.filter((image) => !disabledIds.value.includes(image.id)),
)

function isEnabled(id: SideImageId) {
  return !disabledIds.value.includes(id)
}

function setEnabled(id: SideImageId, enabled: boolean) {
  if (enabled) {
    disabledIds.value = disabledIds.value.filter((item) => item !== id)
    return
  }
  if (!disabledIds.value.includes(id)) {
    disabledIds.value = [...disabledIds.value, id]
  }
}

function pickSideImage(): string | null {
  const pool = enabledImages.value
  if (pool.length === 0) return null
  return pool[Math.floor(Math.random() * pool.length)]!.src
}

function setForcedSideImage(src: string | null) {
  forcedSideImageSrc.value = src
}

function setSideImageDismiss(handler: (() => void) | null) {
  sideImageDismiss.value = handler
}

export function useSideImagePrefs() {
  return {
    enabledImages,
    forcedSideImageSrc,
    sideImageDismiss,
    isEnabled,
    setEnabled,
    pickSideImage,
    setForcedSideImage,
    setSideImageDismiss,
  }
}
