<script setup lang="ts">
import { computed } from 'vue'
import { withBase } from '@/lib/basePath'
import type { TtsProfileTrait } from '@/types/elo'

const props = defineProps<{
  traits: TtsProfileTrait[]
  heading?: string
  subtitle?: string
}>()

const typeNames = new Set(['LI', 'MI', 'HI', 'WB', 'TAG', 'REM', 'SK', 'VH', 'DCD', 'EC', 'ILO', 'IM', 'IL'])
const hiddenChars = new Set(['Not Impetuous', 'No Cube', 'Non Hackable'])
const iconFiles: Record<string, string> = {
  Regular: 'regular.svg',
  Irregular: 'irregular.svg',
  Impetuous: 'impetuous.svg',
  Cube: 'cube.svg',
  'Cube 2.0': 'cube2.svg',
  Hackable: 'hackable.svg',
  Peripheral: 'peripheral.svg',
}
const iconOrder = ['Regular', 'Irregular', 'Impetuous', 'Cube', 'Cube 2.0', 'Hackable', 'Peripheral']

const chars = computed(() => props.traits.filter((trait) => trait.kind === 'char'))
const typeLabel = computed(() => chars.value.filter((trait) => typeNames.has(trait.name)).map((trait) => trait.name).join(' '))
const icons = computed(() =>
  chars.value
    .filter((trait) => iconFiles[trait.name])
    .slice()
    .sort((left, right) => iconOrder.indexOf(left.name) - iconOrder.indexOf(right.name))
    .map((trait) => ({
      name: trait.name,
      src: withBase(`/profile-chars/${iconFiles[trait.name]}`),
    })),
)
const extraChars = computed(() =>
  chars.value
    .filter((trait) => !typeNames.has(trait.name) && !hiddenChars.has(trait.name) && !iconFiles[trait.name])
    .map((trait) => trait.name),
)
const attributes = computed(() => props.traits.filter((trait) => trait.kind === 'attribute'))

const listGroups = [
  { kind: 'weapon', title: 'Armes' },
  { kind: 'equipment', title: 'Équipement' },
  { kind: 'skill', title: 'Compétences' },
] as const

const visibleGroups = computed(() =>
  listGroups
    .map((group) => ({ ...group, line: traitLine(group.kind) }))
    .filter((group) => group.line),
)

function traitLabel(trait: TtsProfileTrait) {
  return trait.detail ? `${trait.name} (${trait.detail})` : trait.name
}

function sortedTraits(kind: string) {
  return props.traits
    .filter((trait) => trait.kind === kind)
    .slice()
    .sort((left, right) => traitLabel(left).localeCompare(traitLabel(right), 'en', { sensitivity: 'base' }))
}

function traitLine(kind: string) {
  return sortedTraits(kind).map(traitLabel).join(', ')
}
</script>

<template>
  <section class="space-y-3">
    <div v-if="heading">
      <h3 class="font-medium">{{ heading }}</h3>
      <p v-if="subtitle" class="text-muted-foreground">{{ subtitle }}</p>
    </div>
    <div v-if="typeLabel || icons.length || extraChars.length" class="flex flex-wrap items-center gap-1.5">
      <span v-if="typeLabel" class="font-medium tracking-wide">{{ typeLabel }}</span>
      <img
        v-for="icon in icons"
        :key="icon.name"
        :src="icon.src"
        :alt="icon.name"
        :title="icon.name"
        class="size-5 shrink-0"
      />
      <span v-for="name in extraChars" :key="name" class="text-muted-foreground">{{ name }}</span>
    </div>
    <div v-if="attributes.length" class="space-y-1">
      <h4 class="font-medium">Caractéristiques</h4>
      <div class="overflow-x-auto">
        <table class="w-full border-collapse text-center text-xs">
          <thead>
            <tr>
              <th
                v-for="trait in attributes"
                :key="trait.name"
                class="border-b border-border px-1 pb-1 font-medium text-muted-foreground"
              >
                {{ trait.name }}
              </th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td v-for="trait in attributes" :key="`${trait.name}-value`" class="px-1 pt-1 font-medium">
                {{ trait.detail || '—' }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
    <div v-for="group in visibleGroups" :key="group.kind">
      <h4 class="mb-1 font-medium">{{ group.title }}</h4>
      <p>{{ group.line }}</p>
    </div>
  </section>
</template>
