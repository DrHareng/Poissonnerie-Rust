<script setup lang="ts">
import { computed } from 'vue'
import { toast } from 'vue-sonner'
import { ClipboardCopy, ExternalLink } from '@lucide/vue'
import { armyListUrl, normalizeArmyListCode } from '@/lib/armyList'
import { Button } from '@/components/ui/button'

const props = withDefaults(
  defineProps<{
    code: string | null | undefined
    iconOnly?: boolean
  }>(),
  { iconOnly: false },
)

const buttonSize = computed(() => (props.iconOnly ? 'icon-sm' : 'sm'))

async function copyCode() {
  const code = normalizeArmyListCode(props.code ?? '')
  if (!code) return
  try {
    await navigator.clipboard.writeText(code)
    toast.success('Code copié')
  } catch {
    toast.error('Impossible de copier le code')
  }
}

function openArmy() {
  const code = normalizeArmyListCode(props.code ?? '')
  if (!code) return
  window.open(armyListUrl(code), '_blank', 'noopener,noreferrer')
}
</script>

<template>
  <div v-if="code?.trim()" class="inline-flex items-center gap-0.5">
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
