import { createApp } from 'vue'
import './style.css'
import App from './App.vue'
import { router } from '@/router'
import { refreshAuth } from '@/composables/useAuth'
import { initNativeApp } from '@/lib/nativeApp'
import { flushPartieOutbox } from '@/lib/partieOffline'

void (async () => {
  await initNativeApp()
  await refreshAuth()
})()

async function syncPendingParties() {
  if (!navigator.onLine) return
  try {
    await flushPartieOutbox()
  } catch {
    /* retry on next online event */
  }
}

window.addEventListener('online', () => {
  void syncPendingParties()
})
void syncPendingParties()

createApp(App).use(router).mount('#app')
