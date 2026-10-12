<script setup lang="ts">
import { ref } from 'vue'
import { ChevronDown, FileText } from '@lucide/vue'
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuTrigger,
} from 'reka-ui'
import { toast } from 'vue-sonner'
import type { CommonRule, ScenarioDetail } from '@/types/elo'
import { buildScenarioDocument } from '@/lib/scenarioDocument'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'

const props = defineProps<{
  scenario: ScenarioDetail
  rules: CommonRule[]
}>()

const exporting = ref(false)

const menuItemClass = cn(
  'relative flex w-full cursor-default select-none items-center gap-2 rounded-md px-2 py-1.5 text-sm outline-none',
  'transition-colors hover:bg-primary/10 hover:text-primary focus:bg-primary/10 focus:text-primary',
)

async function exportAs(format: 'docx' | 'pdf') {
  if (exporting.value) return
  exporting.value = true
  try {
    const doc = buildScenarioDocument(props.scenario, props.rules)
    const missing =
      format === 'docx'
        ? await import('@/lib/scenarioDocx').then((mod) =>
            mod.downloadScenarioDocx(doc),
          )
        : await import('@/lib/scenarioPdf').then((mod) =>
            mod.downloadScenarioPdf(doc),
          )
    if (missing > 0) {
      toast.message(
        missing === 1
          ? 'Une image n’a pas pu être incluse.'
          : 'Certaines images n’ont pas pu être incluses.',
      )
    }
  } catch (error) {
    toast.error(
      error instanceof Error
        ? error.message
        : 'Impossible de générer le document',
    )
  } finally {
    exporting.value = false
  }
}
</script>

<template>
  <DropdownMenuRoot>
    <DropdownMenuTrigger as-child>
      <Button
        type="button"
        variant="ghost"
        size="xs"
        class="shrink-0 text-muted-foreground"
        :disabled="exporting"
        aria-label="Générer un document"
      >
        <FileText class="size-3.5" />
        {{ exporting ? 'Préparation…' : 'Document' }}
        <ChevronDown class="size-3 opacity-70" />
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent
        align="end"
        :side-offset="6"
        class="topbar-user-menu min-w-40"
      >
        <DropdownMenuItem as-child>
          <button
            type="button"
            :class="menuItemClass"
            @click="exportAs('docx')"
          >
            Word (.docx)
          </button>
        </DropdownMenuItem>
        <DropdownMenuItem as-child>
          <button type="button" :class="menuItemClass" @click="exportAs('pdf')">
            PDF
          </button>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>
