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

/**
 * Mode édition admin (bouton crayon / œil en top bar).
 * - `isAdmin` : compte admin (pour afficher le switch).
 * - `isEditMode` / `showAdminUi` : admin + mode édition → afficher les contrôles admin.
 * En mode consultation, l’UI doit être identique à celle d’un non-admin
 * (seul le switch reste visible).
 */
export function useAdminEditMode() {
  const { isAdmin } = useAuth()

  const isEditMode = computed(() => isAdmin.value && editMode.value)

  /** Alias explicite pour les vues (boutons, onglets, formulaires admin). */
  const showAdminUi = computed(() => isEditMode.value)

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
    showAdminUi,
    canEditContent,
    setEditMode,
    toggleEditMode,
  }
}
