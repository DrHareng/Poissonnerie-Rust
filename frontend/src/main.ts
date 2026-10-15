import { createApp } from 'vue'
import './style.css'
import App from './App.vue'
import { router } from '@/router'
import { refreshAuth } from '@/composables/useAuth'
import { initNativeApp } from '@/lib/nativeApp'
import { flushPartieOutbox } from '@/lib/partieOffline'

const CHUNK_RELOAD_KEY = 'poissonnerie-chunk-reload'

window.addEventListener('vite:preloadError', (event) => {
  if (sessionStorage.getItem(CHUNK_RELOAD_KEY) === '1') return
  event.preventDefault()
  sessionStorage.setItem(CHUNK_RELOAD_KEY, '1')
  window.location.reload()
})

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

sessionStorage.removeItem(CHUNK_RELOAD_KEY)
createApp(App).use(router).mount('#app')
