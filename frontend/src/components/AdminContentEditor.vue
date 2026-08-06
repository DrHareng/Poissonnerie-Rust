<script setup lang="ts">
import { computed, ref, watch } from 'vue'
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
    name?: string | null
    rows?: number
    /** Éditeur Markdown avec barre d’outils (défaut: true). */
    markdown?: boolean
    /** Règles pour le sélecteur de liens `[[slug]]`. */
    rules?: CommonRule[]
    /** Éditeur sans liens de règles ni images. */
    simpleMarkdown?: boolean
    persist: (payload: { name?: string; body: string }) => Promise<void>
  }>(),
  {
    name: null,
    rows: 10,
    markdown: true,
    rules: () => [],
    simpleMarkdown: false,
  },
)

const editing = ref(false)
const saving = ref(false)
const draftName = ref('')
const draftBody = ref('')

const isEmpty = computed(() => !props.body?.trim())

watch(
  () => [props.name, props.body] as const,
  () => {
    if (!editing.value) {
      draftName.value = props.name ?? ''
      draftBody.value = props.body
    }
  },
  { immediate: true },
)

watch(
  () => props.canEdit,
  (canEdit) => {
    if (!canEdit && editing.value) {
      cancel()
    }
  },
)

function startEdit() {
  draftName.value = props.name ?? ''
  draftBody.value = props.body
  editing.value = true
}

function cancel() {
  editing.value = false
  draftName.value = props.name ?? ''
  draftBody.value = props.body
}

async function save() {
  if (props.name != null && !draftName.value.trim()) {
    toast.error('Le nom est requis')
    return
  }
  saving.value = true
  try {
    await props.persist({
      ...(props.name != null ? { name: draftName.value.trim() } : {}),
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
  <Button
    v-if="canEdit && !editing"
    type="button"
    variant="outline"
    size="sm"
    class="admin-edit-btn absolute top-3 right-3 z-10"
    @click="startEdit"
  >
    <Pencil class="size-3.5" />
    Éditer
  </Button>

  <div v-if="editing" class="space-y-3">
    <Input
      v-if="name != null"
      v-model="draftName"
      placeholder="Nom"
      autocomplete="off"
    />
    <MarkdownEditor
      v-if="markdown"
      v-model="draftBody"
      :rows="rows"
      :rules="rules"
      :simple="simpleMarkdown"
    />
    <textarea
      v-else
      v-model="draftBody"
      :rows="rows"
      class="flex w-full rounded-md border border-input bg-transparent px-3 py-2 font-mono text-sm shadow-xs outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50"
      spellcheck="false"
    />
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
      <Button
        type="button"
        size="sm"
        :disabled="saving"
        @click="save"
      >
        {{ saving ? 'Enregistrement…' : 'Enregistrer' }}
      </Button>
    </div>
  </div>

  <div
    v-else-if="canEdit && isEmpty"
    class="rounded-md border border-dashed border-border/70 px-3 py-4 text-sm text-muted-foreground"
  >
    Zone vide — cliquez sur Éditer pour renseigner le contenu.
  </div>

  <slot v-else />
</template>
