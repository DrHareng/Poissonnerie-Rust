export const INFINITY_ARMY_LIST_BASE = 'https://infinitytheuniverse.com/army/list/'

const URL_PREFIXES = [
  'https://infinitytheuniverse.com/army/list/',
  'http://infinitytheuniverse.com/army/list/',
  'https://www.infinitytheuniverse.com/army/list/',
  'http://www.infinitytheuniverse.com/army/list/',
]

const FACTION_SLUG_RE = /^[a-z0-9]+(?:-[a-z0-9]+)*$/

/** Extrait le code de liste depuis un code brut ou une URL Army complète. */
export function normalizeArmyListCode(raw: string): string {
  let code = raw.trim()
  const lower = code.toLowerCase()
  for (const marker of ['army/list/', 'army/infinity/list/']) {
    const idx = lower.indexOf(marker)
    if (idx >= 0) {
      code = code.slice(idx + marker.length)
      break
    }
  }
  code = code.split(/[?#]/)[0] ?? code
  code = code.replace(/^\/+/, '').trim()
  try {
    code = decodeURIComponent(code)
  } catch {
    // code déjà décodé ou mal formé : on garde tel quel
  }
  return code.trim()
}

export function armyListUrl(code: string): string {
  const normalized = normalizeArmyListCode(code)
  return `${INFINITY_ARMY_LIST_BASE}${normalized}`
}

/**
 * Décode le code Army 7 (base64) et lit le slug de faction.
 * Format observé : préfixe variable + string longueur-préfixée (slug) + nom de liste.
 */
export function parseArmyListFactionSlug(raw: string): string | null {
  const normalized = normalizeArmyListCode(raw)
  if (!normalized) return null

  let binary: Uint8Array
  try {
    const decoded = decodeURIComponent(normalized)
    binary = base64ToBytes(decoded)
  } catch {
    try {
      binary = base64ToBytes(normalized)
    } catch {
      return null
    }
  }

  if (binary.length < 5) return null

  // Le slug est une string longueur-préfixée ; un préfixe VLQ/version peut précéder.
  for (let offset = 0; offset <= Math.min(6, binary.length - 2); offset++) {
    const length = binary[offset]
    if (length === undefined || length < 3 || length > 64) continue
    if (offset + 1 + length > binary.length) continue

    let slug = ''
    let ok = true
    for (let i = 0; i < length; i++) {
      const byte = binary[offset + 1 + i]!
      if (byte < 0x20 || byte > 0x7e) {
        ok = false
        break
      }
      slug += String.fromCharCode(byte)
    }
    if (!ok) continue
    if (FACTION_SLUG_RE.test(slug)) {
      return slug
    }
  }

  return null
}

function base64ToBytes(value: string): Uint8Array {
  const cleaned = value.replace(/[^A-Za-z0-9+/=_-]/g, '').replace(/-/g, '+').replace(/_/g, '/')
  const padded = cleaned + '='.repeat((4 - (cleaned.length % 4)) % 4)
  const binary = atob(padded)
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i)
  }
  return bytes
}
