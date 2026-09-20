import { computed, ref } from 'vue'

export const CONFIRMATION_RECEIVED_LABEL = 'Confirmation bien reçu'
export const CONFIRMATION_ACK_MS = 900

const pendingIds = ref(new Set<number>())
const receivedIds = ref(new Set<number>())

function withId(source: Set<number>, id: number, add: boolean) {
  const next = new Set(source)
  if (add) next.add(id)
  else next.delete(id)
  return next
}

export function useServerConfirmAck() {
  const pending = computed(() => pendingIds.value)
  const received = computed(() => receivedIds.value)

  function isPending(id: number) {
    return pendingIds.value.has(id)
  }

  function isReceived(id: number) {
    return receivedIds.value.has(id)
  }

  function begin(id: number) {
    if (receivedIds.value.has(id) || pendingIds.value.has(id)) return false
    pendingIds.value = withId(pendingIds.value, id, true)
    return true
  }

  function succeed(id: number) {
    pendingIds.value = withId(pendingIds.value, id, false)
    receivedIds.value = withId(receivedIds.value, id, true)
  }

  function fail(id: number) {
    pendingIds.value = withId(pendingIds.value, id, false)
  }

  return {
    pending,
    received,
    isPending,
    isReceived,
    begin,
    succeed,
    fail,
  }
}

export function waitConfirmationAck() {
  return new Promise<void>((resolve) => {
    window.setTimeout(resolve, CONFIRMATION_ACK_MS)
  })
}
