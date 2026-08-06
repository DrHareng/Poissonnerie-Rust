<script setup lang="ts">
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

defineProps<{
  saving: boolean
  idPrefix?: string
}>()

const localDisplayName = defineModel<string>('displayName', { required: true })
const localAvatarUrl = defineModel<string>('avatarUrl', { required: true })

const emit = defineEmits<{
  save: []
  resetDisplayName: []
  resetAvatar: []
}>()
</script>

<template>
  <form class="flex flex-col gap-4" @submit.prevent="emit('save')">
    <div class="grid gap-2">
      <Label :for="`${idPrefix ?? 'profile'}-display-name`">Pseudo affiché</Label>
      <Input
        :id="`${idPrefix ?? 'profile'}-display-name`"
        v-model="localDisplayName"
        placeholder="Laisser vide pour utiliser le pseudo Discord"
        autocomplete="off"
      />
    </div>
    <div class="grid gap-2">
      <Label :for="`${idPrefix ?? 'profile'}-avatar-url`">URL de l'avatar</Label>
      <Input
        :id="`${idPrefix ?? 'profile'}-avatar-url`"
        v-model="localAvatarUrl"
        placeholder="Laisser vide pour utiliser l'avatar Discord"
        autocomplete="off"
        inputmode="url"
      />
    </div>
    <div class="flex flex-col gap-2">
      <Button type="submit" :disabled="saving">
        {{ saving ? 'Enregistrement...' : 'Enregistrer' }}
      </Button>
      <Button
        type="button"
        variant="outline"
        :disabled="saving"
        @click="emit('resetDisplayName')"
      >
        Restaurer le pseudo Discord
      </Button>
      <Button
        type="button"
        variant="outline"
        :disabled="saving"
        @click="emit('resetAvatar')"
      >
        Restaurer l'avatar Discord
      </Button>
    </div>
  </form>
</template>
