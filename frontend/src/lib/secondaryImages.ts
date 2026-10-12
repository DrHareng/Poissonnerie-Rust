import { withBase } from '@/lib/basePath'

export const SECONDARY_IMAGES: Record<string, string> = {
  enlevement: 'Enlèvement.png',
  'saisie-de-materiel': 'Saisie_de_matériel.png',
  'soif-de-sang': 'Soif_de_sang.png',
  'ciblage-orbital': 'Ciblage_orbital.png',
  'reconnaissance-poussee': 'Reconnaissance.png',
  'vol-informations': "Vol_d'informations.png",
  'tete-de-pont': 'Tête_de_pont.png',
  investigation: 'Investigation.png',
}

export function secondaryImageSrc(
  secondary: string | { slug: string } | null | undefined,
): string | undefined {
  if (!secondary) return undefined
  const slug = typeof secondary === 'string' ? secondary : secondary.slug
  const filename = SECONDARY_IMAGES[slug]
  return filename
    ? withBase(`/secondaires/${encodeURIComponent(filename)}`)
    : undefined
}
