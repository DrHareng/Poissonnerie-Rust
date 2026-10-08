<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useResizeObserver } from '@vueuse/core'
import { toast } from 'vue-sonner'
import { Pencil } from '@lucide/vue'
import {
  normalizeArmyListCode,
  parseArmyListFactionSlug,
  parseArmyListName,
} from '@/lib/armyList'
import ArmyListQuickActions from '@/components/ArmyListQuickActions.vue'
import { useArmies } from '@/composables/useArmies'
import type { Army } from '@/types/elo'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

const props = defineProps<{
  code: string | null | undefined
  currentArmyId?: number | null
  canEdit: boolean
  persist: (code: string, armyId?: number | null) => Promise<void>
}>()

const { ensureLoaded, getArmy, getArmyBySlug } = useArmies()

const editing = ref(false)
const saving = ref(false)
const draft = ref('')
const pendingCode = ref<string | null>(null)
const pendingArmy = ref<Army | null>(null)

const currentArmy = computed(() => getArmy(props.currentArmyId))

const listName = computed(() => parseArmyListName(props.code ?? ''))
const listLabel = computed(() => listName.value ?? 'Liste sans nom')

const nameEl = ref<HTMLElement | null>(null)
const nameTruncated = ref(false)

const nameHover = computed(() =>
  nameTruncated.value ? listLabel.value : undefined,
)

function updateNameTruncation() {
  const el = nameEl.value
  nameTruncated.value = !!el && el.scrollWidth > el.clientWidth + 1
}

useResizeObserver(nameEl, updateNameTruncation)

watch([listLabel, editing], () => {
  void nextTick(updateNameTruncation)
})

watch(
  () => props.code,
  (value) => {
    if (!editing.value) {
      draft.value = value ?? ''
    }
  },
  { immediate: true },
)

onMounted(() => {
  void ensureLoaded()
  void nextTick(updateNameTruncation)
})

function startEdit() {
  draft.value = props.code ?? ''
  pendingCode.value = null
  pendingArmy.value = null
  editing.value = true
}

function cancelEdit() {
  editing.value = false
  draft.value = props.code ?? ''
  pendingCode.value = null
  pendingArmy.value = null
}

function cancelMismatch() {
  pendingCode.value = null
  pendingArmy.value = null
}

async function commit(code: string, armyId?: number | null, successMessage?: string) {
  saving.value = true
  try {
    await props.persist(code, armyId)
    editing.value = false
    pendingCode.value = null
    pendingArmy.value = null
    toast.success(successMessage ?? 'Liste enregistrée')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Enregistrement impossible')
  } finally {
    saving.value = false
  }
}

async function save() {
  saving.value = true
  try {
    await ensureLoaded()
    const code = normalizeArmyListCode(draft.value)
    const slug = code ? parseArmyListFactionSlug(code) : null
    const army = slug ? getArmyBySlug(slug) : undefined

    if (
      army &&
      props.currentArmyId != null &&
      army.id !== props.currentArmyId
    ) {
      pendingCode.value = code
      pendingArmy.value = army
      saving.value = false
      return
    }

    await commit(
      code,
      army?.id ?? null,
      army
        ? `Liste enregistrée · ${army.name}`
        : code && slug
          ? `Liste enregistrée (sectorielle « ${slug} » inconnue)`
          : 'Liste enregistrée',
    )
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Enregistrement impossible')
    saving.value = false
  }
}

async function confirmUpdateArmy() {
  if (pendingCode.value == null || !pendingArmy.value) return
  const army = pendingArmy.value
  await commit(
    pendingCode.value,
    army.id,
    `Liste enregistrée · sectorielle mise à jour : ${army.name}`,
  )
}

</script>

<template>
  <div class="space-y-2">
    <div v-if="editing" class="grid gap-2">
      <div class="flex flex-wrap items-center gap-2">
        <Input
          v-model="draft"
          placeholder="Code ou URL Infinity Army…"
          autocomplete="off"
          spellcheck="false"
          class="min-w-0 flex-1 text-xs"
          :disabled="saving || !!pendingArmy"
        />
        <ArmyListQuickActions :code="draft" />
      </div>

      <Alert v-if="pendingArmy" variant="destructive" class="neon-panel-accent">
        <AlertTitle>Sectorielle différente</AlertTitle>
        <AlertDescription class="space-y-3">
          <p>
            La liste indique
            <span class="font-medium text-foreground">{{ pendingArmy.name }}</span>,
            alors que la partie a été démarrée avec
            <span class="font-medium text-foreground">
              {{ currentArmy?.name ?? 'une autre sectorielle' }}
            </span>.
          </p>
          <div class="flex flex-wrap gap-2">
            <Button
              type="button"
              size="sm"
              variant="outline"
              :disabled="saving"
              @click="cancelMismatch"
            >
              Annuler
            </Button>
            <Button
              type="button"
              size="sm"
              :disabled="saving"
              @click="confirmUpdateArmy"
            >
              {{ saving ? 'Enregistrement…' : 'Modifier la sectorielle' }}
            </Button>
          </div>
        </AlertDescription>
      </Alert>

      <div v-else class="flex flex-wrap gap-2">
        <Button type="button" size="sm" :disabled="saving" @click="save">
          {{ saving ? 'Enregistrement…' : 'Enregistrer' }}
        </Button>
        <Button type="button" size="sm" variant="outline" :disabled="saving" @click="cancelEdit">
          Annuler
        </Button>
      </div>
    </div>

    <div
      v-else
      class="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-center gap-x-1.5 gap-y-1 text-sm"
    >
      <span class="text-muted-foreground">Liste :</span>
      <span
        v-if="code?.trim()"
        ref="nameEl"
        class="min-w-0 truncate font-medium"
        :title="nameHover"
      >
        {{ listLabel }}
      </span>
      <Button
        v-else-if="canEdit"
        type="button"
        size="sm"
        variant="outline"
        class="w-fit"
        @click="startEdit"
      >
        <Pencil class="size-3.5" />
        Saisir le code
      </Button>
      <span v-else class="italic text-muted-foreground">non renseignée</span>
      <div v-if="code?.trim()" class="col-start-2 flex flex-wrap items-center gap-1">
        <ArmyListQuickActions :code="code" />
        <Button
          v-if="canEdit"
          type="button"
          size="sm"
          variant="ghost"
          title="Modifier la liste"
          aria-label="Modifier la liste"
          @click="startEdit"
        >
          <Pencil class="size-3.5" />
        </Button>
      </div>
    </div>
  </div>
</template>
