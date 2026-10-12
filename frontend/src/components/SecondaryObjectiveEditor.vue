<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { Pencil } from '@lucide/vue'
import { toast } from 'vue-sonner'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import MarkdownEditor from '@/components/MarkdownEditor.vue'
import type { CommonRule } from '@/types/elo'

const MAX_IMAGE_BYTES = 8 * 1024 * 1024

const props = withDefaults(
  defineProps<{
    canEdit: boolean
    body: string
    name: string
    imageSrc?: string
    rows?: number
    rules?: CommonRule[]
    /** En grille de cartes, le formulaire prend toute la largeur. */
    expand?: boolean
    persist: (payload: {
      name?: string
      body: string
      image: File | null
    }) => Promise<void>
  }>(),
  {
    imageSrc: undefined,
    rows: 8,
    rules: () => [],
    expand: false,
  },
)

const editing = ref(false)
const saving = ref(false)
const draftName = ref('')
const draftBody = ref('')
const pickedFile = ref<File | null>(null)
const pickedPreview = ref<string | null>(null)
const fileInputKey = ref(0)

const previewSrc = computed(() => pickedPreview.value ?? props.imageSrc ?? '')

watch(
  () => [props.name, props.body] as const,
  () => {
    if (!editing.value) {
      draftName.value = props.name
      draftBody.value = props.body
    }
  },
  { immediate: true },
)

watch(
  () => props.canEdit,
  (canEdit) => {
    if (!canEdit && editing.value) cancel()
  },
)

onBeforeUnmount(clearPicked)

function revokePicked() {
  if (pickedPreview.value) URL.revokeObjectURL(pickedPreview.value)
  pickedPreview.value = null
  pickedFile.value = null
}

function clearPicked() {
  revokePicked()
  fileInputKey.value += 1
}

function startEdit() {
  draftName.value = props.name
  draftBody.value = props.body
  clearPicked()
  editing.value = true
}

function cancel() {
  editing.value = false
  draftName.value = props.name
  draftBody.value = props.body
  clearPicked()
}

function onImageChange(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  revokePicked()
  if (!file) return
  const type = file.type.toLowerCase()
  const accepted =
    type === 'image/png' ||
    type === 'image/jpeg' ||
    type === 'image/gif' ||
    type === 'image/webp' ||
    (!type && /\.(png|jpe?g|gif|webp)$/i.test(file.name))
  if (!accepted) {
    input.value = ''
    toast.error('Format d’image non supporté (png, jpg, gif, webp)')
    return
  }
  if (file.size > MAX_IMAGE_BYTES) {
    input.value = ''
    toast.error('L’image dépasse 8 Mo')
    return
  }
  pickedFile.value = file
  pickedPreview.value = URL.createObjectURL(file)
}

async function save() {
  if (!draftName.value.trim()) {
    toast.error('Le nom est requis')
    return
  }
  saving.value = true
  try {
    await props.persist({
      name: draftName.value.trim(),
      body: draftBody.value,
      image: pickedFile.value,
    })
    editing.value = false
    clearPicked()
  } catch (error) {
    toast.error(
      error instanceof Error ? error.message : 'Impossible d’enregistrer',
    )
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div
    :class="
      expand ? (editing ? 'relative col-span-full' : 'relative') : undefined
    "
  >
    <Button
      v-if="canEdit && !editing"
      type="button"
      variant="outline"
      size="sm"
      class="admin-edit-btn absolute top-3 right-3 z-20"
      @click.stop="startEdit"
    >
      <Pencil class="size-3.5" />
      Éditer
    </Button>

    <div v-if="editing" class="space-y-3" :class="{ 'neon-panel p-4': expand }">
      <Input v-model="draftName" placeholder="Nom" autocomplete="off" />
      <div class="flex flex-wrap items-start gap-4">
        <img
          v-if="previewSrc"
          :src="previewSrc"
          alt=""
          class="max-h-64 w-auto rounded-md border border-border object-contain"
        />
        <div class="min-w-48 flex-1 space-y-2">
          <p class="text-sm font-medium">Image</p>
          <input
            :key="fileInputKey"
            type="file"
            accept="image/png,image/jpeg,image/gif,image/webp,.png,.jpg,.jpeg,.gif,.webp"
            class="border-input file:text-foreground h-8 w-full min-w-0 rounded-lg border bg-transparent px-2.5 py-1 text-sm file:mr-3 file:border-0 file:bg-transparent file:text-sm file:font-medium"
            @change="onImageChange"
          />
          <p class="text-xs text-muted-foreground">
            Sans nouveau fichier, l’image actuelle est conservée. Un fichier
            choisi remplace l’existante.
          </p>
        </div>
      </div>
      <MarkdownEditor v-model="draftBody" :rows="rows" :rules="rules" />
      <div class="flex flex-wrap justify-end gap-2">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          :disabled="saving"
          @click="cancel"
        >
          Annuler
        </Button>
        <Button type="button" size="sm" :disabled="saving" @click="save">
          {{ saving ? 'Enregistrement…' : 'Enregistrer' }}
        </Button>
      </div>
    </div>

    <slot v-else />
  </div>
</template>
