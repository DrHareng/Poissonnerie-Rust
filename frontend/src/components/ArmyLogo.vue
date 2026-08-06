<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useArmies } from '@/composables/useArmies'

const props = withDefaults(
  defineProps<{
    armyId?: number | null
    title?: string
    size?: 'sm' | 'md' | 'lg'
  }>(),
  {
    size: 'sm',
  },
)

const { ensureLoaded, getArmy } = useArmies()

const army = computed(() => getArmy(props.armyId))
const tooltip = computed(() => props.title ?? army.value?.name ?? '')
const sizeClass = computed(() => {
  if (props.size === 'lg') return 'army-logo army-logo--lg'
  if (props.size === 'md') return 'army-logo army-logo--md'
  return 'army-logo'
})

onMounted(() => {
  void ensureLoaded()
})
</script>

<template>
  <img
    v-if="army"
    :src="army.logo_url"
    :alt="army.name"
    :title="tooltip"
    :class="sizeClass"
  />
</template>
