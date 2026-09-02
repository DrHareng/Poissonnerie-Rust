import { computed, type Ref } from 'vue'
import { useElementSize, useWindowSize } from '@vueuse/core'

/** Taille minimale (px) du rendu effectif de l'illustration latérale. */
const MIN_RENDERED_WIDTH = 104
const MIN_RENDERED_HEIGHT = 160

/** Part du corps de page réservée à l'illustration (doit rester alignée avec le CSS). */
const SIDE_PANEL_FRACTION = 1 / 3

/** Garde-fous viewport : en dessous, l'illustration est masquée sans mesure. */
const MIN_BODY_HEIGHT = 380
const MIN_WINDOW_WIDTH = 1200

export function useBrandSideImageVisibility(
  sideImage: Ref<string | null | undefined>,
  customSideActive: Ref<boolean>,
  bodyRef: Ref<HTMLElement | null>,
) {
  const { width: bodyWidth, height: bodyHeight } = useElementSize(bodyRef)
  const { width: windowWidth, height: windowHeight } = useWindowSize()

  const viewportAllowsBrandImage = computed(() => {
    if (windowWidth.value < MIN_WINDOW_WIDTH) return false
    if (windowHeight.value < MIN_BODY_HEIGHT + 160) return false
    if (bodyHeight.value > 0 && bodyHeight.value < MIN_BODY_HEIGHT) return false
    return true
  })

  /** Estime la taille utile de la colonne sans monter/démonter l'image (évite les sauts au zoom). */
  const columnLargeEnough = computed(() => {
    if (bodyWidth.value <= 0 || bodyHeight.value <= 0) return true
    const sideWidth = bodyWidth.value * SIDE_PANEL_FRACTION
    return sideWidth >= MIN_RENDERED_WIDTH && bodyHeight.value >= MIN_RENDERED_HEIGHT
  })

  const viewportTooSmall = computed(() => !viewportAllowsBrandImage.value)

  const showBrandImage = computed(
    () =>
      !customSideActive.value &&
      sideImage.value != null &&
      viewportAllowsBrandImage.value &&
      columnLargeEnough.value,
  )

  return { showBrandImage, viewportTooSmall }
}
