import { onBeforeUnmount, watch, type Ref } from 'vue'

const closers: Array<() => void> = []

export function closeTopOverlay() {
  const close = closers.pop()
  if (!close) return false
  close()
  return true
}

export function watchOverlay(open: Ref<boolean>) {
  watch(open, (shown, _, onCleanup) => {
    if (!shown) return
    const close = () => {
      open.value = false
    }
    closers.push(close)
    const previous = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') open.value = false
    }
    window.addEventListener('keydown', onKey)
    onCleanup(() => {
      const index = closers.lastIndexOf(close)
      if (index >= 0) closers.splice(index, 1)
      window.removeEventListener('keydown', onKey)
      document.body.style.overflow = previous
    })
  })
  onBeforeUnmount(() => {
    open.value = false
  })
}
