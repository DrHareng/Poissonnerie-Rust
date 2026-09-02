import { withBase } from '@/lib/basePath'

/** Incrémenter quand une image dans `public/brand/` change (invalidation cache navigateur). */
const BRAND_REVISION = '20260831'

function brandImage(path: string): string {
  return `${withBase(path)}?v=${BRAND_REVISION}`
}

/** Illustrations du panneau gauche. Les `name` s'affichent dans les préférences joueur. */
export const SIDE_IMAGES = [
  { id: 'side_01', src: brandImage('/brand/side_01.png'), name: 'Lune & Griffes' },
  { id: 'side_02', src: brandImage('/brand/side_02.png'), name: 'Saint Sépulcre' },
  { id: 'side_03', src: brandImage('/brand/side_03.png'), name: 'Écailles de Saint-Nazaire' },
  { id: 'side_04', src: brandImage('/brand/side_04.png'), name: 'Sous la montagne' },
  { id: 'side_05', src: brandImage('/brand/side_05.png'), name: 'Soleil Levant' },
  { id: 'side_06', src: brandImage('/brand/side_06.png'), name: 'Hégémonie' },
  { id: 'side_07', src: brandImage('/brand/side_07.png'), name: 'Filets de l\'Olympe' },
  { id: 'side_08', src: brandImage('/brand/side_08.png'), name: 'Rivages de Jade' },
  { id: 'side_09', src: brandImage('/brand/side_09.png'), name: 'Kosmoflot' },
  { id: 'side_10', src: brandImage('/brand/side_10.png'), name: 'Jardins du Triumvirat' },
  { id: 'side_11', src: brandImage('/brand/side_11.png'), name: 'Au p\'tit optimiste' },
] as const

export type SideImageId = (typeof SIDE_IMAGES)[number]['id']



export function isSideImageId(value: unknown): value is SideImageId {

  return SIDE_IMAGES.some((image) => image.id === value)

}


