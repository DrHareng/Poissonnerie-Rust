<script setup lang="ts">
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { SIDE_IMAGES } from '@/lib/sideImages'
import { useSideImagePrefs } from '@/composables/useSideImagePrefs'

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

const { isEnabled, setEnabled } = useSideImagePrefs()
</script>

<template>
  <div class="flex flex-col gap-4">
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

    <fieldset class="grid gap-2 border-t border-border pt-4">
      <legend class="text-sm font-medium">Illustrations latérales</legend>
      <p class="text-xs text-muted-foreground">
        Cochez celles qui peuvent s'afficher. Les cases s'appliquent tout de
        suite. Si aucune n'est cochée, le contenu est centré.
      </p>
      <label
        v-for="image in SIDE_IMAGES"
        :key="image.id"
        class="flex cursor-pointer items-center gap-3 rounded-md border border-border px-2 py-1.5 hover:bg-primary/5"
      >
        <input
          :id="`${idPrefix ?? 'profile'}-side-image-${image.id}`"
          type="checkbox"
          class="size-4 accent-primary"
          :checked="isEnabled(image.id)"
          @change="
            setEnabled(image.id, ($event.target as HTMLInputElement).checked)
          "
        />
        <img
          :src="image.src"
          alt=""
          class="h-14 w-10 shrink-0 rounded bg-black/40 object-contain"
        />
        <span class="text-sm">{{ image.name }}</span>
      </label>
    </fieldset>
  </div>
</template>
