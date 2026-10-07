import type { Component } from 'vue'
import type { RouteLocationRaw } from 'vue-router'
import {
  BookOpen,
  Map,
  Podium,
  Swords,
  Trophy,
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
    to: '/matchs',
    label: 'Matchs',
    icon: Swords,
    children: [
      { to: { name: 'matchs' }, label: 'Matchs' },
      { to: { name: 'matchs-listes' }, label: 'Listes' },
      { to: { name: 'matchs-cr' }, label: 'Rapports' },
    ],
  },
  {
    to: '/tournois',
    label: 'Tournois',
    icon: Trophy,
    children: [
      { to: { name: 'tournois' }, label: 'En cours' },
      { to: { name: 'tournois-termines' }, label: 'Terminés' },
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
    label: 'Maps & liens',
    icon: BookOpen,
    children: [
      { to: { name: 'maps' }, label: 'Map TTS' },
      { to: { name: 'links' }, label: 'Liens' },
    ],
  },
]

export function isMainNavLinkActive(path: string, to: string) {
  if (to === '/classement') {
    return (
      path === '/classement' ||
      path.startsWith('/sectorielle') ||
      path.startsWith('/joueur')
    )
  }
  if (to === '/matchs') {
    return path.startsWith('/matchs')
  }
  if (to === '/tournois') {
    return path.startsWith('/tournoi')
  }
  if (to === '/scenarios') {
    return path.startsWith('/scenarios')
  }
  if (to === '/maps') {
    return path.startsWith('/maps') || path.startsWith('/links')
  }
  return path === to
}
