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
    to: { name: 'matchs-cr' },
    label: 'Compte rendu',
    activeNames: ['matchs-cr'],
  },
]

export const tournoisTabs: PageTitleTab[] = [
  {
    to: { name: 'tournois' },
    label: 'Tournois',
    activeNames: ['tournois'],
  },
]
