<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
} from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

const props = defineProps<{
  page: number
  totalPages: number
  total: number
  pageSize: number
  loading?: boolean
}>()

const emit = defineEmits<{
  pageChange: [page: number]
}>()

const draft = ref(String(props.page))

watch(
  () => props.page,
  (value) => {
    draft.value = String(value)
  },
)

const pageStart = computed(() => {
  if (!props.total) return 0
  return (props.page - 1) * props.pageSize + 1
})

const pageEnd = computed(() => {
  if (!props.total) return 0
  return Math.min(props.page * props.pageSize, props.total)
})

const inputWidthCh = computed(
  () => Math.max(2, String(Math.max(props.totalPages, props.page, 1)).length) + 2,
)

const atFirst = computed(() => props.page <= 1 || props.loading)
const atLast = computed(
  () => props.page >= props.totalPages || props.loading,
)

function goToPage(nextPage: number) {
  const max = Math.max(1, props.totalPages)
  const clamped = Math.min(max, Math.max(1, Math.trunc(nextPage) || 1))
  if (clamped === props.page) {
    draft.value = String(props.page)
    return
  }
  emit('pageChange', clamped)
}

function commitDraft() {
  const parsed = Number.parseInt(draft.value.trim(), 10)
  if (!Number.isFinite(parsed)) {
    draft.value = String(props.page)
    return
  }
  goToPage(parsed)
}

function onDraftInput(value: string | number) {
  draft.value = String(value).replace(/\D/g, '')
}

function onPageKeydown(event: KeyboardEvent) {
  if (event.key === 'Enter') {
    event.preventDefault()
    commitDraft()
    ;(event.target as HTMLInputElement).blur()
  }
  if (event.key === 'Escape') {
    draft.value = String(props.page)
    ;(event.target as HTMLInputElement).blur()
  }
}

function selectInput(event: Event) {
  const target = event.target
  if (target instanceof HTMLInputElement) target.select()
}
</script>

<template>
  <div
    class="mt-4 flex flex-col gap-3 border-t border-border pt-4 sm:flex-row sm:items-center sm:justify-between"
  >
    <p class="text-sm text-muted-foreground">
      {{ pageStart }}–{{ pageEnd }} sur {{ total }}
    </p>
    <div class="flex items-center gap-1.5">
      <Button
        type="button"
        variant="outline"
        size="icon-sm"
        :disabled="atFirst"
        title="Première page"
        aria-label="Première page"
        @click="goToPage(1)"
      >
        <ChevronsLeft class="size-4" />
      </Button>
      <Button
        type="button"
        variant="outline"
        size="icon-sm"
        :disabled="atFirst"
        title="Page précédente"
        aria-label="Page précédente"
        @click="goToPage(page - 1)"
      >
        <ChevronLeft class="size-4" />
      </Button>
      <div class="flex items-center gap-1.5 px-1">
        <span class="sr-only">Page</span>
        <Input
          :model-value="draft"
          inputmode="numeric"
          pattern="[0-9]*"
          autocomplete="off"
          aria-label="Numéro de page"
          title="Saisir un numéro de page"
          :disabled="loading"
          class="h-7 w-auto min-w-8 px-1 text-center tabular-nums"
          :style="{ width: `${inputWidthCh}ch` }"
          @update:model-value="onDraftInput"
          @keydown="onPageKeydown"
          @focus="selectInput"
          @blur="commitDraft"
        />
        <span class="text-sm tabular-nums text-muted-foreground">
          / {{ totalPages }}
        </span>
      </div>
      <Button
        type="button"
        variant="outline"
        size="icon-sm"
        :disabled="atLast"
        title="Page suivante"
        aria-label="Page suivante"
        @click="goToPage(page + 1)"
      >
        <ChevronRight class="size-4" />
      </Button>
      <Button
        type="button"
        variant="outline"
        size="icon-sm"
        :disabled="atLast"
        title="Dernière page"
        aria-label="Dernière page"
        @click="goToPage(totalPages)"
      >
        <ChevronsRight class="size-4" />
      </Button>
    </div>
  </div>
</template>
