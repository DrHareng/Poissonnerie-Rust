import { onBeforeUnmount, ref } from 'vue'

const customSideActive = ref(false)

export function useAppSidePanelHost() {
  return { customSideActive }
}

export function useAppSidePanel() {
  function setCustomSide(active: boolean) {
    customSideActive.value = active
  }

  onBeforeUnmount(() => {
    customSideActive.value = false
  })

  return { customSideActive, setCustomSide }
}
