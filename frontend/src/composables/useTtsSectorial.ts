import { ref } from 'vue'
import { fetchPrefs, updatePrefs } from '@/lib/api'

const armyId = ref<string>()
let loadPromise: Promise<void> | null = null

export function useTtsSectorial() {
  async function ensureLoaded() {
    if (!loadPromise) {
      loadPromise = fetchPrefs()
        .then((prefs) => {
          if (!armyId.value && prefs.tts_army_id != null) {
            armyId.value = String(prefs.tts_army_id)
          }
        })
        .catch(() => {
          loadPromise = null
        })
    }
    await loadPromise
  }

  function setArmyId(value: string | undefined) {
    if (!value || value === armyId.value) return
    armyId.value = value
    const id = Number(value)
    if (!Number.isInteger(id) || id <= 0) return
    void updatePrefs({ tts_army_id: id }).catch(() => {})
  }

  return { armyId, ensureLoaded, setArmyId }
}
