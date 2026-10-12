<script setup lang="ts">
import { ref, watch } from 'vue'
import { Pencil } from '@lucide/vue'
import { toast } from 'vue-sonner'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import MarkdownEditor from '@/components/MarkdownEditor.vue'
import type { CommonRule } from '@/types/elo'

const props = withDefaults(
  defineProps<{
    canEdit: boolean
    body: string
    name: string
    rows?: number
    rules?: CommonRule[]
    /** En grille de cartes, le formulaire prend toute la largeur. */
    expand?: boolean
    persist: (payload: { name?: string; body: string }) => Promise<void>
  }>(),
  {
    rows: 8,
    rules: () => [],
    expand: false,
  },
)

const editing = ref(false)
const saving = ref(false)
const draftName = ref('')
const draftBody = ref('')

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

function startEdit() {
  draftName.value = props.name
  draftBody.value = props.body
  editing.value = true
}

function cancel() {
  editing.value = false
  draftName.value = props.name
  draftBody.value = props.body
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
    })
    editing.value = false
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
