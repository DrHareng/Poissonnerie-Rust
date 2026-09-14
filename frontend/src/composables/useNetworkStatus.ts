import { useOnline } from '@vueuse/core'

export function useNetworkStatus() {
  const isOnline = useOnline()
  return { isOnline }
}
