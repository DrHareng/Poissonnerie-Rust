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

export const matchsTabs: PageTitleTab[] = [
  {
    to: { name: 'matchs' },
    label: 'Matchs',
    activeNames: ['matchs'],
  },
  {
    to: { name: 'matchs-listes' },
    label: 'Listes',
    activeNames: ['matchs-listes'],
  },
  {
    to: { name: 'matchs-cr' },
    label: 'Rapports',
    activeNames: ['matchs-cr'],
  },
]

export const tournoisTabs: PageTitleTab[] = [
  {
    to: { name: 'tournois' },
    label: 'En cours',
    activeNames: ['tournois'],
  },
  {
    to: { name: 'tournois-termines' },
    label: 'Terminés',
    activeNames: ['tournois-termines'],
  },
]

export const adminTabs: PageTitleTab[] = [
  {
    to: { name: 'admin' },
    label: 'Administration',
    activeNames: ['admin'],
  },
  {
    to: { name: 'admin-tts-reports' },
    label: 'Signalements TTS',
    activeNames: ['admin-tts-reports'],
  },
]
