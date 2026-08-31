import type { Army } from '@/types/elo'

const NON_PLAYABLE_SLUGS = new Set(['non-aligned-armies', 'contracted-back-up'])

/** Sectorielles jouables, aligné sur `is_listable` côté Rust. */
export function isListableArmy(army: Pick<Army, 'id' | 'slug'>): boolean {
  if (army.id % 100 === 99) return false
  return !NON_PLAYABLE_SLUGS.has(army.slug.trim().toLowerCase())
}
