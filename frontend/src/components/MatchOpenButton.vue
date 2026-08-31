<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { Eye } from '@lucide/vue'
import { Button, type ButtonVariants } from '@/components/ui/button'

const props = withDefaults(
  defineProps<{
    matchId: number
    variant?: ButtonVariants['variant']
    size?: ButtonVariants['size']
    title?: string
  }>(),
  {
    variant: 'ghost',
    size: 'icon-sm',
  },
)

const label = computed(
  () => props.title ?? `Voir le match #${props.matchId}`,
)
</script>

<template>
  <Button as-child :variant="variant" :size="size">
    <RouterLink
      :to="{ name: 'match', params: { id: String(matchId) } }"
      :title="label"
      :aria-label="label"
    >
      <Eye class="size-4" />
    </RouterLink>
  </Button>
</template>
