<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import {
  Bold,
  Eye,
  Heading3,
  Italic,
  Link2,
  List,
  Type,
} from '@lucide/vue'
import { fetchScenarioContentImages } from '@/lib/api'
import { Button } from '@/components/ui/button'
import MarkdownContent from '@/components/MarkdownContent.vue'
import type { CommonRule } from '@/types/elo'
import { cn } from '@/lib/utils'

const props = withDefaults(
  defineProps<{
    modelValue: string
    rows?: number
    rules?: CommonRule[]
    placeholder?: string
    /** Masque les liens de règles et l’insertion d’images. */
    simple?: boolean
  }>(),
  {
    rows: 10,
    rules: () => [],
    placeholder: '',
    simple: false,
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const textareaRef = ref<HTMLTextAreaElement | null>(null)
const showPreview = ref(false)
const rulePick = ref('')
const imagePick = ref('')
const scenarioImages = ref<string[]>([])
const savedSelection = ref<{ start: number; end: number } | null>(null)

const sortedRules = computed(() =>
  [...props.rules].sort((a, b) =>
    a.name.localeCompare(b.name, 'fr', { sensitivity: 'base' }),
  ),
)

onMounted(async () => {
  if (props.simple) return
  try {
    scenarioImages.value = await fetchScenarioContentImages()
  } catch {
    scenarioImages.value = []
  }
})

watch(rulePick, (slug) => {
  if (!slug) return
  insertRuleRef(slug)
  rulePick.value = ''
})

watch(imagePick, (filename) => {
  if (!filename) return
  insertImage(filename)
  imagePick.value = ''
})

function sync(value: string) {
  emit('update:modelValue', value)
}

function rememberSelection() {
  const el = textareaRef.value
  if (!el) return
  savedSelection.value = {
    start: el.selectionStart,
    end: el.selectionEnd,
  }
}

function selectionRange(): { start: number; end: number } {
  const el = textareaRef.value
  if (el && document.activeElement === el) {
    return { start: el.selectionStart, end: el.selectionEnd }
  }
  if (savedSelection.value) return savedSelection.value
  const len = props.modelValue.length
  return { start: len, end: len }
}

function focusAndSelect(start: number, end: number) {
  nextTick(() => {
    const el = textareaRef.value
    if (!el) return
    el.focus()
    el.setSelectionRange(start, end)
    savedSelection.value = { start, end }
  })
}

function wrapSelection(before: string, after: string, placeholder = 'texte') {
  const value = props.modelValue
  const { start, end } = selectionRange()
  const selected = value.slice(start, end)
  const inner = selected || placeholder
  const next = value.slice(0, start) + before + inner + after + value.slice(end)
  sync(next)
  const innerStart = start + before.length
  focusAndSelect(innerStart, innerStart + inner.length)
}

function toggleLinePrefix(prefix: string) {
  const value = props.modelValue
  const { start, end } = selectionRange()
  const lineStart = value.lastIndexOf('\n', start - 1) + 1
  let lineEnd = value.indexOf('\n', end)
  if (lineEnd < 0) lineEnd = value.length
  const line = value.slice(lineStart, lineEnd)
  const trimmed = line.replace(/^\s+/, '')
  const leading = line.slice(0, line.length - trimmed.length)

  let nextLine: string
  let cursorOffset: number
  if (trimmed.startsWith(prefix)) {
    nextLine = leading + trimmed.slice(prefix.length)
    cursorOffset = -prefix.length
  } else {
    const withoutHeading = trimmed.replace(/^#{1,3}\s+/, '')
    nextLine = leading + prefix + withoutHeading
    cursorOffset = prefix.length - (trimmed.length - withoutHeading.length)
  }

  const next = value.slice(0, lineStart) + nextLine + value.slice(lineEnd)
  sync(next)
  focusAndSelect(
    Math.max(lineStart, start + cursorOffset),
    Math.max(lineStart, end + cursorOffset),
  )
}

function insertListItem() {
  const value = props.modelValue
  const { start } = selectionRange()
  const lineStart = value.lastIndexOf('\n', start - 1) + 1
  const atLineStart = start === lineStart
  const prefix = atLineStart ? '- ' : '\n- '
  const next = value.slice(0, start) + prefix + value.slice(start)
  sync(next)
  focusAndSelect(start + prefix.length, start + prefix.length)
}

function insertRuleRef(slug: string, label?: string) {
  const value = props.modelValue
  const { start, end } = selectionRange()
  const selected = value.slice(start, end).trim()
  const display = label ?? selected
  const token = display ? `[[${slug}|${display}]]` : `[[${slug}]]`
  const next = value.slice(0, start) + token + value.slice(end)
  sync(next)
  focusAndSelect(start + token.length, start + token.length)
}

function insertBlankRuleRef() {
  const value = props.modelValue
  const { start, end } = selectionRange()
  const selected = value.slice(start, end).trim()
  if (selected) {
    const token = `[[slug|${selected}]]`
    const next = value.slice(0, start) + token + value.slice(end)
    sync(next)
    focusAndSelect(start + 2, start + 6)
    return
  }
  wrapSelection('[[', ']]', 'slug|libellé')
}

function insertImage(filename: string) {
  const value = props.modelValue
  const { start, end } = selectionRange()
  const token = `[img]${filename}[img]`
  const before = value.slice(0, start)
  const after = value.slice(end)
  const needsLeadingNewline = before.length > 0 && !before.endsWith('\n')
  const needsTrailingNewline = after.length > 0 && !after.startsWith('\n')
  const block =
    (needsLeadingNewline ? '\n' : '') +
    token +
    (needsTrailingNewline ? '\n' : '')
  const next = before + block + after
  sync(next)
  const cursor = before.length + block.length
  focusAndSelect(cursor, cursor)
}

function onKeydown(event: KeyboardEvent) {
  if (!(event.ctrlKey || event.metaKey) || event.altKey) return
  const key = event.key.toLowerCase()
  if (key === 'b' && !event.shiftKey) {
    event.preventDefault()
    wrapSelection('**', '**')
  } else if (key === 'i' && !event.shiftKey) {
    event.preventDefault()
    wrapSelection('*', '*')
  } else if (key === 'b' && event.shiftKey) {
    event.preventDefault()
    wrapSelection('=b=', '=b=')
  } else if (key === 'v' && event.shiftKey) {
    event.preventDefault()
    wrapSelection('=v=', '=v=')
  } else if (key === 'r' && event.shiftKey) {
    event.preventDefault()
    wrapSelection('=r=', '=r=')
  } else if (key === 'o' && event.shiftKey) {
    event.preventDefault()
    wrapSelection('=o=', '=o=')
  }
}
</script>

<template>
  <div class="md-editor space-y-2">
    <div
      class="md-editor-toolbar flex flex-wrap items-center gap-1 rounded-md border border-input bg-muted/30 p-1"
      role="toolbar"
      aria-label="Mise en forme Markdown"
    >
      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        title="Titre de section (###)"
        @mousedown.prevent
        @click="toggleLinePrefix('### ')"
      >
        <Heading3 />
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        title="Titre inline (###…###)"
        @mousedown.prevent
        @click="wrapSelection('###', '###')"
      >
        <Type />
      </Button>

      <span class="mx-0.5 h-4 w-px bg-border" aria-hidden="true" />

      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        title="Gras (**…**) — Ctrl+B"
        @mousedown.prevent
        @click="wrapSelection('**', '**')"
      >
        <Bold />
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        title="Italique (*…*) — Ctrl+I"
        @mousedown.prevent
        @click="wrapSelection('*', '*')"
      >
        <Italic />
      </Button>

      <span class="mx-0.5 h-4 w-px bg-border" aria-hidden="true" />

      <Button
        type="button"
        variant="ghost"
        size="xs"
        class="min-w-6 px-1.5 font-semibold"
        style="color: #00e5ff"
        title="Couleur bleue (=b=…=b=) — Ctrl+Shift+B"
        @mousedown.prevent
        @click="wrapSelection('=b=', '=b=')"
      >
        B
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="xs"
        class="min-w-6 px-1.5 font-semibold"
        style="color: #39ff88"
        title="Couleur verte (=v=…=v=) — Ctrl+Shift+V"
        @mousedown.prevent
        @click="wrapSelection('=v=', '=v=')"
      >
        V
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="xs"
        class="min-w-6 px-1.5 font-semibold"
        style="color: #ff2ed1"
        title="Couleur rose (=r=…=r=) — Ctrl+Shift+R"
        @mousedown.prevent
        @click="wrapSelection('=r=', '=r=')"
      >
        R
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="xs"
        class="min-w-6 px-1.5 font-semibold"
        style="color: #ff9500"
        title="Couleur orange (=o=…=o=) — Ctrl+Shift+O"
        @mousedown.prevent
        @click="wrapSelection('=o=', '=o=')"
      >
        O
      </Button>

      <span class="mx-0.5 h-4 w-px bg-border" aria-hidden="true" />

      <template v-if="!simple">
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          title="Lien de règle ([[slug|libellé]])"
          @mousedown.prevent
          @click="insertBlankRuleRef"
        >
          <Link2 />
        </Button>
        <select
          v-if="sortedRules.length > 0"
          v-model="rulePick"
          class="md-editor-rule-pick h-6 max-w-[10rem] rounded-[min(var(--radius-md),10px)] border border-transparent bg-transparent px-1 text-xs text-muted-foreground outline-none hover:bg-muted focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
          title="Insérer une règle existante"
          @mousedown="rememberSelection"
        >
          <option value="">Règle…</option>
          <option
            v-for="rule in sortedRules"
            :key="rule.slug"
            :value="rule.slug"
          >
            {{ rule.name }}
          </option>
        </select>

        <select
          v-if="scenarioImages.length > 0"
          v-model="imagePick"
          class="md-editor-rule-pick h-6 max-w-[10rem] rounded-[min(var(--radius-md),10px)] border border-transparent bg-transparent px-1 text-xs text-muted-foreground outline-none hover:bg-muted focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
          title="Insérer une image de scénario ([img]fichier[img])"
          @mousedown="rememberSelection"
        >
          <option value="">Image…</option>
          <option
            v-for="name in scenarioImages"
            :key="name"
            :value="name"
          >
            {{ name }}
          </option>
        </select>
      </template>

      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        title="Puce de liste"
        @mousedown.prevent
        @click="insertListItem"
      >
        <List />
      </Button>

      <span class="mx-0.5 h-4 w-px bg-border" aria-hidden="true" />

      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        :class="cn(showPreview && 'bg-muted text-foreground')"
        title="Aperçu"
        :aria-pressed="showPreview"
        @mousedown.prevent
        @click="showPreview = !showPreview"
      >
        <Eye />
      </Button>
    </div>

    <textarea
      ref="textareaRef"
      :value="modelValue"
      :rows="rows"
      :placeholder="placeholder"
      class="flex w-full rounded-md border border-input bg-transparent px-3 py-2 font-mono text-sm shadow-xs outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50"
      spellcheck="false"
      @input="sync(($event.target as HTMLTextAreaElement).value)"
      @select="rememberSelection"
      @keyup="rememberSelection"
      @mouseup="rememberSelection"
      @keydown="onKeydown"
    />

    <div
      v-if="showPreview"
      class="md-editor-preview rounded-md border border-input bg-muted/20 px-3 py-2"
    >
      <p class="mb-2 text-xs font-medium tracking-wide text-muted-foreground uppercase">
        Aperçu
      </p>
      <MarkdownContent
        v-if="modelValue.trim()"
        :source="modelValue"
        :rules="simple ? [] : rules"
      />
      <p v-else class="text-sm text-muted-foreground italic">
        (vide)
      </p>
    </div>
  </div>
</template>
