import type { Component } from 'vue'
import type { RouteLocationRaw } from 'vue-router'
import {
  BookOpen,
  Laptop,
  Map,
  Podium,
  Swords,
} from '@lucide/vue'

export type MainNavChild = {
  to: RouteLocationRaw
  label: string
}

export type MainNavLink = {
  to: string
  label: string
  icon: Component
  children?: MainNavChild[]
  /** Masqué tant que le compte connecté n’est pas admin. */
  adminOnly?: boolean
}

/** Liens principaux partagés (topbar desktop + menu accueil mobile). */
export const mainNavLinks: MainNavLink[] = [
  {
    to: '/scenarios',
    label: 'Scénarios',
    icon: Map,
    children: [
      { to: { name: 'scenarios' }, label: 'Scénarios' },
      {
        to: { name: 'scenarios', query: { tab: 'secondaires' } },
        label: 'Secondaires',
      },
      { to: { name: 'scenarios', query: { tab: 'regles' } }, label: 'Règles' },
    ],
  },
  {
    to: '/tournois',
    label: 'Jeu',
    icon: Swords,
    children: [
      { to: { name: 'tournois' }, label: 'Tournois' },
      { to: { name: 'matchs' }, label: 'Parties' },
      { to: { name: 'matchs-listes' }, label: 'Listes' },
      { to: { name: 'matchs-cr' }, label: 'Rapports' },
    ],
  },
  {
    to: '/classement',
    label: 'Classement',
    icon: Podium,
    children: [
      { to: { name: 'classement' }, label: 'Joueurs' },
      { to: { name: 'sectorielles' }, label: 'Sectorielles' },
    ],
  },
  {
    to: '/maps',
    label: 'TTS',
    icon: Laptop,
    children: [
      { to: { name: 'maps' }, label: 'Maps' },
      { to: { name: 'profils-figs-figs' }, label: 'Figurines' },
      { to: { name: 'profils-figs-listes' }, label: 'Listes' },
    ],
  },
  {
    to: '/links',
    label: 'Autre',
    icon: BookOpen,
    children: [
      { to: { name: 'links' }, label: 'Liens' },
      { to: { name: 'applis' }, label: 'Applications' },
    ],
  },
]

export function visibleMainNavLinks(isAdmin: boolean): MainNavLink[] {
  return mainNavLinks.filter((link) => !link.adminOnly || isAdmin)
}

export function isMainNavLinkActive(path: string, to: string) {
  if (to === '/classement') {
    return (
      path === '/classement' ||
      path.startsWith('/sectorielle') ||
      path.startsWith('/joueur')
    )
  }
  if (to === '/tournois') {
    return path.startsWith('/tournoi') || path.startsWith('/matchs')
  }
  if (to === '/scenarios') {
    return path.startsWith('/scenarios')
  }
  if (to === '/maps') {
    return path.startsWith('/maps') || path.startsWith('/profils-figs')
  }
  if (to === '/links') {
    return path.startsWith('/links') || path.startsWith('/applis')
  }
  return path === to
}
