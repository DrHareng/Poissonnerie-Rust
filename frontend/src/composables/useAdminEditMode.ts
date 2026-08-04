import { computed, ref, watch } from 'vue'
import { useAuth } from '@/composables/useAuth'

const STORAGE_KEY = 'poissonnerie-admin-edit-mode'

function readStored(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === '1'
  } catch {
    return false
  }
}

const editMode = ref(readStored())

watch(editMode, (value) => {
  try {
    localStorage.setItem(STORAGE_KEY, value ? '1' : '0')
  } catch {
    /* ignore */
  }
})

export function useAdminEditMode() {
  const { isAdmin } = useAuth()

  const isEditMode = computed(() => isAdmin.value && editMode.value)

  /** Droits d’édition contenu (scénarios, secondaires, etc.). */
  const canEditContent = computed(() => isEditMode.value)

  function setEditMode(value: boolean) {
    editMode.value = value
  }

  function toggleEditMode() {
    editMode.value = !editMode.value
  }

  return {
    isAdmin,
    isEditMode,
    canEditContent,
    setEditMode,
    toggleEditMode,
  }
}
