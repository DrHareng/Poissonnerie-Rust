<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { onKeyStroke, useScrollLock } from '@vueuse/core'
import { X } from '@lucide/vue'
import { createTtsMapReport } from '@/lib/api'
import { refreshTtsMapReportCount } from '@/composables/useTtsMapReportCount'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Label } from '@/components/ui/label'

const open = defineModel<boolean>('open', { default: false })

const props = defineProps<{
  mapId: number
  mapName: string
}>()

const description = ref('')
const file = ref<File | null>(null)
const previewUrl = ref<string | null>(null)
const fileInput = ref<HTMLInputElement | null>(null)
const submitting = ref(false)

const canSubmit = computed(() => description.value.trim().length > 0 && !submitting.value)

const body = typeof document !== 'undefined' ? document.body : null
const isLocked = useScrollLock(body)
watch(
  open,
  (value) => {
    isLocked.value = value
    if (!value) reset()
  },
  { immediate: true },
)

watch(file, (next) => {
  if (previewUrl.value) URL.revokeObjectURL(previewUrl.value)
  previewUrl.value = next ? URL.createObjectURL(next) : null
})

onUnmounted(() => {
  isLocked.value = false
  if (previewUrl.value) URL.revokeObjectURL(previewUrl.value)
})

function reset() {
  description.value = ''
  file.value = null
  if (fileInput.value) fileInput.value.value = ''
  submitting.value = false
}

function close() {
  open.value = false
}

function onFileChange(event: Event) {
  const input = event.target as HTMLInputElement
  file.value = input.files?.[0] ?? null
}

function onBackdropClick(event: MouseEvent) {
  if (event.target === event.currentTarget) close()
}

onKeyStroke('Escape', (event) => {
  if (!open.value) return
  event.preventDefault()
  close()
})

async function submit() {
  const bodyText = description.value.trim()
  if (!bodyText) {
    toast.error('La description est requise')
    return
  }
  submitting.value = true
  try {
    await createTtsMapReport(props.mapId, bodyText, file.value)
    toast.success('Signalement envoyé')
    close()
    void refreshTtsMapReportCount()
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible d’envoyer le signalement',
    )
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="player-detail-overlay"
      role="dialog"
      aria-modal="true"
      aria-labelledby="tts-report-title"
      @click="onBackdropClick"
    >
      <Card class="player-detail-modal neon-panel w-full max-w-lg">
        <CardHeader class="relative pr-12">
          <CardTitle id="tts-report-title">Remonter un soucis</CardTitle>
          <CardDescription>
            Décrivez le problème constaté sur « {{ mapName }} ».
          </CardDescription>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            class="absolute top-3 right-3"
            aria-label="Fermer"
            @click="close"
          >
            <X class="size-4" />
          </Button>
        </CardHeader>
        <CardContent class="space-y-4">
          <div class="grid gap-2">
            <Label for="tts-report-description">Description</Label>
            <textarea
              id="tts-report-description"
              v-model="description"
              rows="6"
              class="flex w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-xs outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
              maxlength="8000"
              placeholder="Expliquez ce qui ne va pas…"
            />
          </div>
          <div class="grid gap-2">
            <Label for="tts-report-image">Image (optionnel)</Label>
            <input
              id="tts-report-image"
              ref="fileInput"
              type="file"
              accept="image/png,image/jpeg,image/webp,image/gif"
              class="text-sm text-muted-foreground file:mr-3 file:rounded-md file:border-0 file:bg-primary/15 file:px-3 file:py-1.5 file:text-sm file:font-medium file:text-primary"
              @change="onFileChange"
            />
            <img
              v-if="previewUrl"
              :src="previewUrl"
              alt="Aperçu"
              class="max-h-48 w-full rounded-md object-contain"
            />
          </div>
          <div class="flex justify-end gap-2">
            <Button type="button" variant="ghost" size="sm" @click="close">
              Annuler
            </Button>
            <Button
              type="button"
              size="sm"
              :disabled="!canSubmit"
              @click="submit"
            >
              {{ submitting ? 'Envoi…' : 'Envoyer' }}
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  </Teleport>
</template>
