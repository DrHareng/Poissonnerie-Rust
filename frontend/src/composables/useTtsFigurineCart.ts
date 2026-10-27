import { computed, ref } from 'vue'
import type { TtsFigurine, TtsFigurineLoadout } from '@/types/elo'

export type TtsFigurineCartLine = {
  key: string
  figurine_id: number
  nom: string
  painter: string
  photo_url: string | null
  updated_at: number
  asset_bundle_url: string
  profile_id: number
  form_id: number | null
  loadout: string
  marker_name: string | null
  differences: string
  /** Couleur de socle RRGGBB. */
  color: string
}

const items = ref<TtsFigurineCartLine[]>([])
const open = ref(false)
let lineSeq = 0

function normalizeCartColor(raw: string) {
  const text = raw.trim().replace(/^#/, '').toUpperCase()
  if (/^[0-9A-F]{6}$/.test(text)) return text
  if (/^[0-9A-F]{3}$/.test(text)) {
    return text
      .split('')
      .map((char) => char + char)
      .join('')
  }
  return 'FFFFFF'
}

export function useTtsFigurineCart() {
  const count = computed(() => items.value.length)
  const hasItems = computed(() => items.value.length > 0)
  const reopenVisible = computed(() => hasItems.value && !open.value)

  function openCart() {
    open.value = true
  }

  function closeCart() {
    open.value = false
  }

  function addLine(figurine: TtsFigurine, loadout: TtsFigurineLoadout, color = 'FFFFFF') {
    lineSeq += 1
    items.value = [
      ...items.value,
      {
        key: `cart-${lineSeq}-${figurine.id}-${loadout.profile_id}-${loadout.form_id ?? 'p'}`,
        figurine_id: figurine.id,
        nom: figurine.nom,
        painter: figurine.painter,
        photo_url: figurine.photo_url,
        updated_at: figurine.updated_at,
        asset_bundle_url: figurine.asset_bundle_url,
        profile_id: loadout.profile_id,
        form_id: loadout.form_id ?? null,
        loadout: loadout.loadout,
        marker_name: loadout.marker_name,
        differences: loadout.differences,
        color: normalizeCartColor(color),
      },
    ]
    open.value = true
  }

  function setLineColor(key: string, color: string) {
    const next = normalizeCartColor(color)
    items.value = items.value.map((line) =>
      line.key === key ? { ...line, color: next } : line,
    )
  }

  function applyColorToAll(color: string) {
    const next = normalizeCartColor(color)
    items.value = items.value.map((line) => ({ ...line, color: next }))
  }

  function removeLine(key: string) {
    items.value = items.value.filter((line) => line.key !== key)
    if (items.value.length === 0) open.value = false
  }

  function clearCart() {
    items.value = []
    open.value = false
  }

  return {
    items,
    open,
    count,
    hasItems,
    reopenVisible,
    openCart,
    closeCart,
    addLine,
    setLineColor,
    applyColorToAll,
    removeLine,
    clearCart,
  }
}
