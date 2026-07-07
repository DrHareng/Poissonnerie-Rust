<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { Fish } from '@lucide/vue'
import { addPlayer } from '@/lib/api'
import { useAuth } from '@/composables/useAuth'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

const emit = defineEmits<{
  created: []
  cancel: []
}>()

const { user, refresh } = useAuth()

const discordUsername = ref('')
const name = ref('')
const loading = ref(false)

watch(
  user,
  (current) => {
    if (!current) return
    if (!discordUsername.value.trim()) {
      discordUsername.value = current.username
    }
    if (!name.value.trim()) {
      name.value = current.display_name
    }
  },
  { immediate: true },
)

onMounted(() => {
  void refresh()
})

async function submit() {
  const trimmedName = name.value.trim()
  const trimmedDiscordUsername = discordUsername.value.trim()

  if (!trimmedDiscordUsername) {
    toast.error('Indiquez un pseudo Discord.')
    return
  }
  if (!trimmedName) {
    toast.error('Indiquez un pseudo.')
    return
  }

  loading.value = true
  try {
    const player = await addPlayer({
      name: trimmedName,
      discord_username: trimmedDiscordUsername,
    })
    toast.success(`${player.name} ajouté avec ${Math.round(player.rating)} ELO.`)
    discordUsername.value = ''
    name.value = ''
    await refresh()
    emit('created')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Erreur inconnue')
  } finally {
    loading.value = false
  }
}

function cancel() {
  discordUsername.value = user.value?.username ?? ''
  name.value = user.value?.display_name ?? ''
  emit('cancel')
}
</script>

<template>
  <Card class="neon-panel">
    <CardHeader>
      <CardTitle class="flex items-center gap-2">
        <Fish class="size-5 text-primary" />
        Nouveau joueur
      </CardTitle>
      <CardDescription>
        Chaque joueur démarre avec 1200 points ELO et est lié à un pseudo Discord.
      </CardDescription>
    </CardHeader>
    <CardContent>
      <form class="flex flex-col gap-4" @submit.prevent="submit">
        <div class="grid gap-2">
          <Label for="player-discord-username">Pseudo Discord</Label>
          <Input
            id="player-discord-username"
            v-model="discordUsername"
            placeholder="Ex. drhareng, kantain45"
            autocomplete="off"
            autocapitalize="off"
            spellcheck="false"
          />
        </div>
        <div class="grid gap-2">
          <Label for="player-name">Pseudo</Label>
          <Input
            id="player-name"
            v-model="name"
            placeholder="Ex. Sardine, Thon..."
            autocomplete="off"
          />
        </div>
        <div class="flex flex-col gap-2 sm:flex-row sm:justify-end">
          <Button type="submit" :disabled="loading">
            {{ loading ? 'Ajout...' : 'Ajouter le joueur' }}
          </Button>
          <Button
            type="button"
            variant="outline"
            :disabled="loading"
            @click="cancel"
          >
            Annuler
          </Button>
        </div>
      </form>
    </CardContent>
  </Card>
</template>
