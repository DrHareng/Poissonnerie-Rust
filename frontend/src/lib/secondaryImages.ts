import { withBase } from '@/lib/basePath'

export type SecondaryImageRef = {
  slug: string
  image_url?: string | null
}

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

function staticSecondaryImageSrc(slug: string): string | undefined {
  const filename = SECONDARY_IMAGES[slug]
  return filename
    ? withBase(`/secondaires/${encodeURIComponent(filename)}`)
    : undefined
}

export function secondaryImageSrc(
  secondary: string | SecondaryImageRef | null | undefined,
): string | undefined {
  if (!secondary) return undefined
  if (typeof secondary !== 'string') {
    if (secondary.image_url) return withBase(secondary.image_url)
    return staticSecondaryImageSrc(secondary.slug)
  }
  return staticSecondaryImageSrc(secondary)
}
