import { computed, ref } from 'vue'
import { fetchArmies } from '@/lib/api'
import type { Army } from '@/types/elo'

const armies = ref<Army[]>([])
let loadPromise: Promise<void> | null = null

export function useArmies() {
  const armiesById = computed(
    () => new Map(armies.value.map((army) => [army.id, army])),
  )

  async function ensureLoaded() {
    if (armies.value.length > 0) {
      return
    }

    if (!loadPromise) {
      loadPromise = fetchArmies()
        .then((data) => {
          armies.value = data
        })
        .catch(() => {
          loadPromise = null
          throw new Error('Impossible de charger les sectorielles')
        })
    }

    await loadPromise
  }

  function getArmy(armyId?: number | null) {
    if (!armyId) return undefined
    return armiesById.value.get(armyId)
  }

  return {
    armies,
    armiesById,
    ensureLoaded,
    getArmy,
  }
}
