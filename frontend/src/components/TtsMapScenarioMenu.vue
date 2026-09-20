<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ChevronDown, Download, Map } from '@lucide/vue'
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from 'reka-ui'
import { fetchTtsMapVariants } from '@/lib/api'
import { withBase } from '@/lib/basePath'
import { cn } from '@/lib/utils'
import type { TtsMapVariant } from '@/types/elo'
import { Button } from '@/components/ui/button'

const props = defineProps<{
  scenarioId: number
}>()

const variants = ref<TtsMapVariant[]>([])

const visible = computed(() => variants.value.length > 0)

const menuItemClass = cn(
  'relative flex w-full cursor-default select-none items-center gap-2 rounded-md px-2 py-1.5 text-sm outline-none',
  'transition-colors hover:bg-primary/10 hover:text-primary focus:bg-primary/10 focus:text-primary',
)

function mapLabel(variant: TtsMapVariant) {
  if (variant.tournament_name) {
    return `${variant.map_name} · ${variant.tournament_name}`
  }
  return variant.map_name
}

async function load() {
  try {
    variants.value = await fetchTtsMapVariants({
      scenarioId: props.scenarioId,
    })
  } catch {
    variants.value = []
  }
}

watch(
  () => props.scenarioId,
  () => {
    void load()
  },
  { immediate: true },
)
</script>

<template>
  <DropdownMenuRoot v-if="visible">
    <DropdownMenuTrigger as-child>
      <Button
        type="button"
        variant="ghost"
        size="xs"
        class="text-muted-foreground"
        aria-label="Maps TTS"
      >
        Maps TTS
        <ChevronDown class="size-3 opacity-70" />
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent
        align="end"
        :side-offset="6"
        class="topbar-user-menu min-w-52"
      >
        <template
          v-for="(variant, index) in variants"
          :key="variant.id"
        >
          <DropdownMenuSeparator
            v-if="index > 0"
            class="topbar-user-menu-separator"
          />
          <DropdownMenuItem as-child>
            <RouterLink
              :to="{ name: 'maps', query: { map: variant.map_slug } }"
              :class="menuItemClass"
            >
              <Map class="size-4" />
              {{ mapLabel(variant) }}
            </RouterLink>
          </DropdownMenuItem>
          <DropdownMenuItem as-child>
            <a
              :href="withBase(variant.json_url)"
              :class="menuItemClass"
            >
              <Download class="size-4" />
              Télécharger le JSON
            </a>
          </DropdownMenuItem>
        </template>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>
