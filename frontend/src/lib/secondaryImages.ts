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

export function secondaryImageSrc(slug: string): string | undefined {
  const filename = SECONDARY_IMAGES[slug]
  return filename ? `/secondaires/${encodeURIComponent(filename)}` : undefined
}
