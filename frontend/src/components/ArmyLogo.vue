<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useArmies } from '@/composables/useArmies'

const props = defineProps<{
  armyId?: number | null
  title?: string
}>()

const { ensureLoaded, getArmy } = useArmies()

const army = computed(() => getArmy(props.armyId))
const tooltip = computed(() => props.title ?? army.value?.name ?? '')

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
    class="army-logo"
  />
</template>
