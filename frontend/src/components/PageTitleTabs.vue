<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, type RouteLocationRaw } from 'vue-router'
import type { PageTitleTab } from '@/lib/pageTitleTabs'

const props = defineProps<{
  tabs: PageTitleTab[]
  ariaLabel: string
  /** Onglet détail courant, affiché après les onglets parents avec le préfixe « > ». */
  current?: { label: string; title?: string; to?: RouteLocationRaw } | null
}>()

type DisplayTab = PageTitleTab & {
  isCurrent?: boolean
  title?: string
  detailLabel?: string
}

const route = useRoute()

const displayTabs = computed((): DisplayTab[] => {
  const label = props.current?.label?.trim()
  if (!label) return props.tabs

  const prefixed = label.startsWith('>') ? label : `> ${label}`
  return [
    ...props.tabs,
    {
      to: props.current?.to ?? route.fullPath,
      label: prefixed,
      detailLabel: label,
      title: props.current?.title,
      activeNames: [],
      isCurrent: true,
    },
  ]
})

const activeLabel = computed(() => {
  if (props.current?.label?.trim()) {
    const label = props.current.label.trim()
    return label.startsWith('>') ? label.slice(1).trim() : label
  }
  const current = route.name
  if (typeof current !== 'string') return props.tabs[0]?.label ?? ''
  return (
    props.tabs.find((tab) => tab.activeNames.includes(current))?.label ??
    props.tabs[0]?.label ??
    ''
  )
})

function isActive(tab: DisplayTab) {
  if (props.current?.label?.trim()) {
    return Boolean(tab.isCurrent)
  }
  return typeof route.name === 'string' && tab.activeNames.includes(route.name)
}
</script>

<template>
  <nav class="page-title-tabs" :aria-label="ariaLabel">
    <div class="page-title-tabs-list">
      <h1 class="sr-only">{{ activeLabel }}</h1>
      <RouterLink
        v-for="tab in displayTabs"
        :key="tab.label"
        :to="tab.to"
        class="page-title-tab"
        :class="{
          'page-title-tab--active': isActive(tab),
          'page-title-tab--detail': tab.isCurrent,
        }"
        :aria-current="isActive(tab) ? 'page' : undefined"
        :title="tab.title ?? tab.detailLabel"
      >
        <template v-if="tab.isCurrent && tab.detailLabel">
          <span class="page-title-tab-detail">&gt; {{ tab.detailLabel }}</span>
        </template>
        <template v-else>{{ tab.label }}</template>
      </RouterLink>
    </div>
    <div v-if="$slots.actions" class="page-title-tabs-actions">
      <slot name="actions" />
    </div>
  </nav>
</template>
