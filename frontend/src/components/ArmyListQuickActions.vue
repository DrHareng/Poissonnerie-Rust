<script setup lang="ts">
import { computed } from 'vue'
import { toast } from 'vue-sonner'
import { ClipboardCopy, ExternalLink } from '@lucide/vue'
import { armyListUrl, normalizeArmyListCode } from '@/lib/armyList'
import { copyTextToClipboard } from '@/lib/utils'
import { Button } from '@/components/ui/button'

const props = withDefaults(
  defineProps<{
    code: string | null | undefined
    iconOnly?: boolean
  }>(),
  { iconOnly: false },
)

const normalized = computed(() => normalizeArmyListCode(props.code ?? ''))

const buttonSize = computed(() => (props.iconOnly ? 'icon-sm' : 'sm'))

async function copyCode() {
  if (!normalized.value) return
  try {
    await copyTextToClipboard(normalized.value)
    toast.success('Code copié')
  } catch {
    toast.error('Impossible de copier le code')
  }
}

function openArmy() {
  if (!normalized.value) return
  window.open(armyListUrl(normalized.value), '_blank', 'noopener,noreferrer')
}
</script>

<template>
  <div
    v-if="normalized"
    class="inline-flex shrink-0 items-center gap-0.5"
  >
    <Button
      type="button"
      :size="buttonSize"
      variant="outline"
      title="Copier le code de la liste"
      :aria-label="iconOnly ? 'Copier le code de la liste' : undefined"
      @click="copyCode"
    >
      <ClipboardCopy class="size-3.5" />
      <span v-if="!iconOnly">Copier</span>
    </Button>
    <Button
      type="button"
      :size="buttonSize"
      variant="outline"
      title="Ouvrir l'Army Builder Infinity"
      :aria-label="iconOnly ? 'Ouvrir l\'Army Builder Infinity' : undefined"
      @click="openArmy"
    >
      <ExternalLink class="size-3.5" />
      <span v-if="!iconOnly">Army</span>
    </Button>
  </div>
</template>
