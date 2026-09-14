<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import AppLayout from '@/layouts/AppLayout.vue'
import { getApiOrigin, setApiOrigin, withBase } from '@/lib/basePath'
import { isNativeApp } from '@/lib/nativeApp'
import { refreshAuth } from '@/composables/useAuth'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

const native = isNativeApp()
const checking = ref(native)
const needsSetup = ref(false)
const apiOrigin = ref(getApiOrigin() || 'http://127.0.0.1:5173/infinity')
const error = ref('')

async function pingApi(): Promise<boolean> {
  try {
    const response = await fetch(`${getApiOrigin()}/api/health`, {
      cache: 'no-store',
    })
    return response.ok
  } catch {
    return false
  }
}

async function checkApi() {
  if (!native) {
    checking.value = false
    return
  }
  checking.value = true
  error.value = ''
  const ok = await pingApi()
  needsSetup.value = !ok
  checking.value = false
  if (ok) {
    await refreshAuth()
  }
}

async function saveOrigin() {
  setApiOrigin(apiOrigin.value)
  await checkApi()
  if (needsSetup.value) {
    error.value =
      'API injoignable. Vérifiez l’URL (ex. http://51.255.44.29/infinity) et votre connexion réseau.'
  }
}

function onNativeAuth() {
  void refreshAuth()
}

onMounted(() => {
  window.addEventListener('poissonnerie-native-auth', onNativeAuth)
  void checkApi()
})

onUnmounted(() => {
  window.removeEventListener('poissonnerie-native-auth', onNativeAuth)
})
</script>

<template>
  <div
    v-if="native && (checking || needsSetup)"
    class="relative flex min-h-dvh flex-col items-center justify-end overflow-hidden bg-black text-white"
  >
    <img
      :src="withBase('/brand/logo.png')"
      alt=""
      class="absolute inset-0 size-full object-cover"
    />
    <div class="absolute inset-0 bg-gradient-to-t from-black/85 via-black/35 to-black/20" />
    <div class="relative z-10 flex w-full max-w-md flex-col items-center gap-3 px-6 pb-12 pt-8 text-center">
      <img
        :src="withBase('/brand/favicon.png')"
        alt="La Poissonnerie"
        class="size-14 object-contain drop-shadow"
      />
      <p v-if="checking" class="text-sm text-white/80">Connexion au serveur…</p>
      <form
        v-else
        class="flex w-full flex-col gap-3 text-left"
        @submit.prevent="saveOrigin"
      >
        <p class="text-sm text-white/80">
          Le téléphone ne joint pas l’API. Vérifiez l’URL de prod ou votre connexion.
        </p>
        <Input v-model="apiOrigin" placeholder="http://51.255.44.29/infinity" />
        <p v-if="error" class="text-sm text-red-300">{{ error }}</p>
        <Button type="submit">Réessayer</Button>
      </form>
    </div>
  </div>
  <AppLayout v-else />
</template>
