<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import { cn } from "@/lib/utils"

const props = defineProps<{
  class?: HTMLAttributes["class"]
}>()
</script>

<template>
  <div
    data-slot="card-footer"
    :class="cn('bg-muted/50 rounded-b-xl border-t p-3 group-data-[size=sm]/card:p-2.5 flex items-center', props.class)"
  >
    <slot />
  </div>
</template>
