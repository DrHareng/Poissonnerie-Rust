<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { onKeyStroke, useScrollLock } from '@vueuse/core'
import { Info, X } from '@lucide/vue'
import {
  createMapUpdate,
  createScenarioUpdate,
  fetchMapUpdates,
  fetchScenarioUpdates,
} from '@/lib/api'
import { DEFAULT_SCENARIO_PACK_SLUG, type ContentUpdate } from '@/types/elo'
import { useAdminEditMode } from '@/composables/useAdminEditMode'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Label } from '@/components/ui/label'

const props = defineProps<{
  kind: 'map' | 'scenario'
  mapId?: number
  scenarioSlug?: string
  refreshKey?: number
}>()

const emit = defineEmits<{
  visible: [value: boolean]
  published: []
}>()

const { canEditContent } = useAdminEditMode()

const updates = ref<ContentUpdate[]>([])
const ready = ref(false)
const dialogOpen = ref(false)
const draft = ref('')
const submitting = ref(false)
const tipOpen = ref(false)
const tipStyle = ref<Record<string, string>>({})
const trigger = ref<HTMLButtonElement | null>(null)

let requestId = 0
let openTimer: ReturnType<typeof setTimeout> | null = null
let closeTimer: ReturnType<typeof setTimeout> | null = null

const latest = computed(() => updates.value[0] ?? null)
const showBadge = computed(() => {
  if (!ready.value) return false
  if (latest.value) return true
  return props.kind === 'scenario' && canEditContent.value
})
const canSubmit = computed(
  () => draft.value.trim().length > 0 && !submitting.value,
)
const subject = computed(() => (props.kind === 'map' ? 'cette map' : 'ce scénario'))

const body = typeof document !== 'undefined' ? document.body : null
const isLocked = useScrollLock(body)

watch(dialogOpen, (value) => {
  isLocked.value = value
  if (!value) {
    draft.value = ''
    submitting.value = false
  }
})

watch(showBadge, (value) => emit('visible', value), { immediate: true })

watch(
  () => [props.kind, props.mapId, props.scenarioSlug] as const,
  () => {
    dialogOpen.value = false
    hideTip()
    void load()
  },
  { immediate: true },
)

watch(
  () => props.refreshKey,
  () => {
    void load(true)
  },
)

onKeyStroke('Escape', (event) => {
  if (!dialogOpen.value) return
  event.preventDefault()
  dialogOpen.value = false
})

if (typeof window !== 'undefined') {
  window.addEventListener('scroll', onScrollOrResize, true)
  window.addEventListener('resize', onScrollOrResize)
}

onBeforeUnmount(() => {
  requestId += 1
  emit('visible', false)
  isLocked.value = false
  clearTipTimers()
  window.removeEventListener('scroll', onScrollOrResize, true)
  window.removeEventListener('resize', onScrollOrResize)
})

function formatDate(timestamp: number) {
  return new Intl.DateTimeFormat('fr-FR', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  }).format(new Date(timestamp * 1000))
}

function formatDateTime(timestamp: number) {
  return new Intl.DateTimeFormat('fr-FR', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(timestamp * 1000))
}

function clearTipTimers() {
  if (openTimer) {
    clearTimeout(openTimer)
    openTimer = null
  }
  if (closeTimer) {
    clearTimeout(closeTimer)
    closeTimer = null
  }
}

function hideTip() {
  clearTipTimers()
  tipOpen.value = false
}

function positionTip() {
  const rect = trigger.value?.getBoundingClientRect()
  if (!rect) return
  const margin = 8
  const maxWidth = 320
  const left = Math.min(
    Math.max(margin, rect.left),
    window.innerWidth - maxWidth - margin,
  )
  tipStyle.value = {
    left: `${left}px`,
    top: `${rect.bottom + margin}px`,
    maxWidth: `${maxWidth}px`,
  }
}

function scheduleTip() {
  if (!latest.value || dialogOpen.value) return
  clearTipTimers()
  openTimer = setTimeout(() => {
    positionTip()
    tipOpen.value = true
  }, 180)
}

function scheduleTipClose() {
  clearTipTimers()
  closeTimer = setTimeout(() => {
    tipOpen.value = false
  }, 120)
}

function onScrollOrResize() {
  if (!tipOpen.value) return
  positionTip()
}

function openDialog() {
  hideTip()
  dialogOpen.value = true
}

function onBackdropClick(event: MouseEvent) {
  if (event.target === event.currentTarget) dialogOpen.value = false
}

async function load(background = false) {
  const id = ++requestId
  const mapId = props.mapId
  const scenarioSlug = props.scenarioSlug
  if (props.kind === 'map' && mapId == null) return
  if (props.kind === 'scenario' && !scenarioSlug) return

  if (!background) ready.value = false
  try {
    const items =
      props.kind === 'map'
        ? await fetchMapUpdates(mapId!)
        : await fetchScenarioUpdates(DEFAULT_SCENARIO_PACK_SLUG, scenarioSlug!)
    if (id !== requestId) return
    updates.value = items
  } catch (error) {
    if (id !== requestId) return
    updates.value = []
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de charger les mises à jour',
    )
  } finally {
    if (id === requestId) ready.value = true
  }
}

async function submit() {
  const description = draft.value.trim()
  if (!description) {
    toast.error('La description est requise')
    return
  }
  submitting.value = true
  try {
    const created =
      props.kind === 'map'
        ? await createMapUpdate(props.mapId!, description)
        : await createScenarioUpdate(
            DEFAULT_SCENARIO_PACK_SLUG,
            props.scenarioSlug!,
            description,
          )
    updates.value = [created, ...updates.value.filter((item) => item.id !== created.id)]
    dialogOpen.value = false
    emit('published')
    toast.success('Mise à jour publiée')
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de publier la mise à jour',
    )
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <div v-if="showBadge" class="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
    <template v-if="latest">
      <span class="min-w-0 truncate">
        Dernière mise à jour :
        <time
          class="text-foreground"
          :datetime="new Date(latest.created_at * 1000).toISOString()"
        >{{ formatDate(latest.created_at) }}</time>
      </span>
      <button
        ref="trigger"
        type="button"
        class="content-update-info"
        aria-label="Voir les mises à jour"
        @pointerenter="scheduleTip"
        @pointerleave="scheduleTipClose"
        @focus="scheduleTip"
        @blur="scheduleTipClose"
        @click="openDialog"
      >
        <Info class="size-3" />
      </button>
    </template>
    <Button
      v-else
      type="button"
      variant="outline"
      size="xs"
      @click="openDialog"
    >
      Ajouter une mise à jour
    </Button>
  </div>

  <Teleport to="body">
    <div
      v-if="tipOpen && latest"
      class="md-rule-tooltip neon-panel pointer-events-auto"
      :style="tipStyle"
      role="tooltip"
      @pointerenter="clearTipTimers"
      @pointerleave="scheduleTipClose"
    >
      <time
        class="mb-1 block text-xs font-medium text-primary"
        :datetime="new Date(latest.created_at * 1000).toISOString()"
      >
        {{ formatDateTime(latest.created_at) }}
      </time>
      <p class="whitespace-pre-wrap break-words text-sm text-foreground/90">
        {{ latest.description }}
      </p>
    </div>

    <div
      v-if="dialogOpen"
      class="player-detail-overlay"
      role="dialog"
      aria-modal="true"
      aria-labelledby="content-updates-title"
      @click="onBackdropClick"
    >
      <Card class="player-detail-modal neon-panel w-full max-w-lg">
        <CardHeader class="relative pr-12">
          <CardTitle id="content-updates-title">Mises à jour</CardTitle>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            class="absolute top-3 right-3"
            aria-label="Fermer"
            @click="dialogOpen = false"
          >
            <X class="size-4" />
          </Button>
        </CardHeader>
        <CardContent class="space-y-4">
          <form
            v-if="canEditContent && kind === 'scenario'"
            class="space-y-2"
            @submit.prevent="submit"
          >
            <Label for="content-update-description">Description</Label>
            <textarea
              id="content-update-description"
              v-model="draft"
              rows="4"
              maxlength="4000"
              class="flex w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-xs outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
              placeholder="Décrivez la mise à jour…"
            />
            <div class="flex justify-end">
              <Button type="submit" size="sm" :disabled="!canSubmit">
                {{ submitting ? 'Publication…' : 'Publier' }}
              </Button>
            </div>
          </form>

          <p v-if="updates.length === 0" class="text-sm text-muted-foreground">
            Aucune mise à jour pour {{ subject }}.
          </p>
          <ol v-else class="space-y-3" :class="{ 'border-t border-border/60 pt-4': canEditContent && kind === 'scenario' }">
            <li
              v-for="update in updates"
              :key="update.id"
              class="space-y-1"
            >
              <time
                class="text-xs font-medium text-primary"
                :datetime="new Date(update.created_at * 1000).toISOString()"
              >
                {{ formatDateTime(update.created_at) }}
              </time>
              <p class="whitespace-pre-wrap break-words text-sm text-foreground/90">
                {{ update.description }}
              </p>
            </li>
          </ol>
        </CardContent>
      </Card>
    </div>
  </Teleport>
</template>
