/** Une seule valeur : camo-0, camo-3, camo-6, imp-1, imp-2, puis normale. */
export function silhouetteKind(description: string): string {
  const tokens = description
    .replace(/\[[^\]]*\]/g, '')
    .replace(/-{2,}/g, ' ● ')
    .split('●')
    .map((part) => part.replace(/\s+/g, '').toLowerCase())
    .filter(Boolean)

  const starts = (prefix: string) => tokens.some((token) => token.startsWith(prefix))
  const has = (needle: string) => tokens.some((token) => token.includes(needle))
  const camo = starts('camouflage')
  const mimetism = starts('mimetism')
  const impersonation = tokens.some(
    (token) =>
      token === 'impersonation' ||
      (token.startsWith('impersonation(') && !token.includes('imp-2') && !token.includes('l2')),
  )
  const impersonationL2 = tokens.some(
    (token) => token.startsWith('impersonation') && (token.includes('imp-2') || token.includes('l2')),
  )

  if (camo && !mimetism) return 'camo-0'
  if (camo && starts('mimetism(-3)')) return 'camo-3'
  if (camo && starts('mimetism(-6)')) return 'camo-6'
  if (impersonation) return 'imp-1'
  if (impersonationL2 || has('killerhackingdevice') || has('hackingdeviceplus') || has('cybermask')) {
    return 'imp-2'
  }
  return 'normale'
}
