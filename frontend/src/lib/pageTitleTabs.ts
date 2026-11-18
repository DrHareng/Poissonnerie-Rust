import type { RouteLocationRaw } from 'vue-router'

export type PageTitleTab = {
  to: RouteLocationRaw
  label: string
  /** Noms de routes qui activent cet onglet. */
  activeNames: string[]
}

export const classementTabs: PageTitleTab[] = [
  {
    to: { name: 'classement' },
    label: 'Joueurs',
    activeNames: ['classement'],
  },
  {
    to: { name: 'sectorielles' },
    label: 'Sectorielles',
    activeNames: ['sectorielles'],
  },
]

/** Tournois / parties / listes / rapports. */
export const jeuTabs: PageTitleTab[] = [
  {
    to: { name: 'tournois' },
    label: 'Tournois',
    activeNames: ['tournois', 'tournoi'],
  },
  {
    to: { name: 'matchs' },
    label: 'Parties',
    activeNames: ['matchs', 'match'],
  },
  {
    to: { name: 'matchs-listes' },
    label: 'Listes',
    activeNames: ['matchs-listes'],
  },
  {
    to: { name: 'matchs-cr' },
    label: 'Rapports',
    activeNames: ['matchs-cr', 'match-cr'],
  },
]

export const ttsTabs: PageTitleTab[] = [
  {
    to: { name: 'maps' },
    label: 'Maps',
    activeNames: ['maps'],
  },
  {
    to: { name: 'profils-figs-figs' },
    label: 'Figurines',
    activeNames: ['profils-figs-figs'],
  },
  {
    to: { name: 'profils-figs-listes' },
    label: 'Listes',
    activeNames: ['profils-figs-listes'],
  },
]

export const autreTabs: PageTitleTab[] = [
  {
    to: { name: 'links' },
    label: 'Liens',
    activeNames: ['links'],
  },
  {
    to: { name: 'applis' },
    label: 'Applications',
    activeNames: ['applis'],
  },
]

export const adminTabs: PageTitleTab[] = [
  {
    to: { name: 'admin' },
    label: 'Administration',
    activeNames: ['admin'],
  },
  {
    to: { name: 'admin-pack' },
    label: 'Pack de missions',
    activeNames: ['admin-pack'],
  },
  {
    to: { name: 'admin-tts-reports' },
    label: 'Signalements TTS',
    activeNames: ['admin-tts-reports'],
  },
  {
    to: { name: 'admin-unit-profiles' },
    label: 'Profils TTS',
    activeNames: ['admin-unit-profiles'],
  },
]
