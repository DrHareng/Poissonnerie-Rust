<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { onKeyStroke, useScrollLock } from '@vueuse/core'
import { Eye, ImagePlus, X } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { SIDE_IMAGES, getSideImage, isSideImageId, type SideImageId } from '@/lib/sideImages'
import { useSideImagePrefs } from '@/composables/useSideImagePrefs'

defineProps<{
  saving: boolean
  idPrefix?: string
}>()

const localDisplayName = defineModel<string>('displayName', { required: true })
const localAvatarUrl = defineModel<string>('avatarUrl', { required: true })
const profileImageId = defineModel<string | null>('profileImageId', { required: true })

const emit = defineEmits<{
  save: []
  resetDisplayName: []
  resetAvatar: []
  clearProfileImage: []
  previewProfileImage: []
}>()

const { isEnabled, setEnabled } = useSideImagePrefs()

const pickerOpen = ref(false)
const draftImageId = ref<SideImageId | null>(null)

const selectedImage = computed(() => getSideImage(profileImageId.value))

const body = typeof document !== 'undefined' ? document.body : null
const isLocked = useScrollLock(body)
watch(
  pickerOpen,
  (value) => {
    isLocked.value = value
    if (value) {
      draftImageId.value = isSideImageId(profileImageId.value)
        ? profileImageId.value
        : null
    }
  },
  { immediate: true },
)

onUnmounted(() => {
  isLocked.value = false
})

function openPicker() {
  pickerOpen.value = true
}

function closePicker() {
  pickerOpen.value = false
}

function onBackdropClick(event: MouseEvent) {
  if (event.target === event.currentTarget) closePicker()
}

function selectDraft(id: SideImageId) {
  draftImageId.value = draftImageId.value === id ? null : id
}

function confirmPicker() {
  profileImageId.value = draftImageId.value
  closePicker()
}

function openPreview() {
  if (!selectedImage.value) return
  emit('previewProfileImage')
}

onKeyStroke('Escape', (event) => {
  if (!pickerOpen.value) return
  event.preventDefault()
  closePicker()
})
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

      <fieldset class="grid gap-2">
        <legend class="text-sm font-medium">Image de profil</legend>
        <p class="text-xs text-muted-foreground">
          Illustration affichée à gauche lorsque quelqu’un consulte votre page
          joueur (à la place du tirage au sort).
        </p>
        <div class="flex items-start gap-3">
          <div
            class="flex h-28 w-20 shrink-0 items-center justify-center overflow-hidden rounded-md border border-border bg-black/40"
          >
            <img
              v-if="selectedImage"
              :src="selectedImage.src"
              :alt="selectedImage.name"
              class="h-full w-full object-cover object-top"
            />
            <span v-else class="px-1 text-center text-[10px] text-muted-foreground">
              Aucune
            </span>
          </div>
          <div class="flex min-w-0 flex-1 flex-col gap-2">
            <p
              v-if="selectedImage"
              class="truncate text-sm font-medium"
              :title="selectedImage.name"
            >
              {{ selectedImage.name }}
            </p>
            <p v-else class="text-sm text-muted-foreground">
              Pas d’image sélectionnée
            </p>
            <div class="flex flex-col gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                :disabled="saving"
                @click="openPicker"
              >
                <ImagePlus class="size-3.5" />
                Choisir votre image de profil
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                :disabled="saving || !selectedImage"
                @click="openPreview"
              >
                <Eye class="size-3.5" />
                Prévisualiser
              </Button>
              <Button
                v-if="profileImageId"
                type="button"
                variant="ghost"
                size="sm"
                :disabled="saving"
                @click="emit('clearProfileImage')"
              >
                Retirer
              </Button>
            </div>
          </div>
        </div>
      </fieldset>

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

    <Teleport to="body">
      <div
        v-if="pickerOpen"
        class="player-detail-overlay"
        role="dialog"
        aria-modal="true"
        aria-labelledby="profile-image-picker-title"
        @click="onBackdropClick"
      >
        <Card class="player-detail-modal neon-panel flex max-h-[min(85vh,52rem)] w-full max-w-6xl! flex-col overflow-hidden!">
          <CardHeader class="relative shrink-0 pr-12">
            <CardTitle id="profile-image-picker-title">
              Choisir votre image de profil
            </CardTitle>
            <CardDescription>
              Cette illustration sera affichée à gauche pour les visiteurs de
              votre page.
            </CardDescription>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              class="absolute top-3 right-3"
              aria-label="Fermer"
              @click="closePicker"
            >
              <X class="size-4" />
            </Button>
          </CardHeader>
          <CardContent class="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden">
            <div class="min-h-0 flex-1 overflow-y-auto pr-1">
              <div class="grid grid-cols-3 gap-2 sm:grid-cols-4 md:grid-cols-5">
                <button
                  v-for="image in SIDE_IMAGES"
                  :key="image.id"
                  type="button"
                  class="overflow-hidden rounded-md border bg-black/40 transition-colors"
                  :class="
                    draftImageId === image.id
                      ? 'border-primary ring-2 ring-primary/40'
                      : 'border-border hover:border-primary/50'
                  "
                  :title="image.name"
                  :aria-pressed="draftImageId === image.id"
                  @click="selectDraft(image.id)"
                >
                  <img
                    :src="image.src"
                    :alt="image.name"
                    class="aspect-[3/4] w-full object-cover object-top"
                  />
                </button>
              </div>
            </div>
            <div class="flex shrink-0 justify-end gap-2 border-t border-border pt-3">
              <Button type="button" variant="ghost" size="sm" @click="closePicker">
                Annuler
              </Button>
              <Button type="button" size="sm" @click="confirmPicker">
                Enregistrer
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>
    </Teleport>
  </div>
</template>
