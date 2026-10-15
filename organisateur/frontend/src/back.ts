import { App as CapApp } from '@capacitor/app'
import { Capacitor } from '@capacitor/core'
import { router } from '@/router'

export async function handleBack() {
  if (router.currentRoute.value.path !== '/') {
    const previous = window.history.state?.back
    if (typeof previous === 'string') {
      router.back()
      return
    }
    await router.replace('/')
    return
  }
  if (Capacitor.getPlatform() === 'android') {
    await CapApp.minimizeApp()
  }
}

export function listenForHardwareBack() {
  if (!Capacitor.isNativePlatform()) return
  void CapApp.addListener('backButton', () => {
    void handleBack()
  })
}
