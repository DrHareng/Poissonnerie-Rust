<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { Pencil } from '@lucide/vue'
import { normalizeArmyListCode, parseArmyListFactionSlug } from '@/lib/armyList'
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
      <Input
        v-model="draft"
        placeholder="Code ou URL Infinity Army…"
        autocomplete="off"
        spellcheck="false"
        class="text-xs"
        :disabled="saving || !!pendingArmy"
      />

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

    <div v-else class="flex flex-wrap items-center gap-2">
      <span class="text-sm text-muted-foreground">Liste :</span>
      <template v-if="code?.trim()">
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
      </template>
      <Button
        v-else-if="canEdit"
        type="button"
        size="sm"
        variant="outline"
        @click="startEdit"
      >
        <Pencil class="size-3.5" />
        Saisir le code
      </Button>
      <span v-else class="text-sm italic text-muted-foreground">non renseignée</span>
    </div>
  </div>
</template>
