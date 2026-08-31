import { createRouter, createWebHistory } from 'vue-router'
import AccueilPage from '@/pages/AccueilPage.vue'
import ClassementPage from '@/pages/ClassementPage.vue'
import JoueurPage from '@/pages/JoueurPage.vue'
import MatchsPage from '@/pages/MatchsPage.vue'
import MatchPage from '@/pages/MatchPage.vue'
import MatchReportPage from '@/pages/MatchReportPage.vue'
import SectoriellesPage from '@/pages/SectoriellesPage.vue'
import SectoriellePage from '@/pages/SectoriellePage.vue'
import ScenariosPage from '@/pages/ScenariosPage.vue'
import TournoisPage from '@/pages/TournoisPage.vue'
import TournoiPage from '@/pages/TournoiPage.vue'
import PartiePage from '@/pages/PartiePage.vue'
import { pageTitle } from '@/lib/pageTitle'

export const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  scrollBehavior(to) {
    if (to.hash) {
      return { el: to.hash, top: 88, behavior: 'smooth' }
    }
    return { top: 0 }
  },
  routes: [
    {
      path: '/',
      name: 'accueil',
      component: AccueilPage,
      meta: { title: 'Accueil' },
    },
    {
      path: '/classement',
      name: 'classement',
      component: ClassementPage,
      meta: { title: 'Classement' },
    },
    {
      path: '/sectorielles',
      name: 'sectorielles',
      component: SectoriellesPage,
      meta: { title: 'Sectorielles' },
    },
    {
      path: '/sectorielle/:id',
      name: 'sectorielle',
      component: SectoriellePage,
      props: true,
    },
    {
      path: '/scenarios',
      name: 'scenarios',
      component: ScenariosPage,
      meta: { title: 'Scénarios' },
    },
    {
      path: '/scenarios/secondaires',
      redirect: { name: 'scenarios', query: { tab: 'secondaires' } },
    },
    {
      path: '/scenarios/:slug',
      redirect: (to) => ({
        name: 'scenarios',
        query: { scenario: String(to.params.slug) },
      }),
    },
    {
      path: '/matchs',
      name: 'matchs',
      component: MatchsPage,
      meta: { title: 'Matchs' },
    },
    {
      path: '/matchs/listes',
      name: 'matchs-listes',
      component: MatchsPage,
      meta: { title: 'Listes' },
    },
    {
      path: '/matchs/cr',
      name: 'matchs-cr',
      component: MatchsPage,
      meta: { title: 'Compte rendu' },
    },
    {
      path: '/matchs/:id',
      name: 'match',
      component: MatchPage,
      props: true,
    },
    {
      path: '/matchs/:id/cr',
      name: 'match-cr',
      component: MatchReportPage,
      props: true,
    },
    {
      path: '/partie',
      name: 'partie',
      component: PartiePage,
      meta: { title: 'Partie' },
    },
    {
      path: '/partie/:id',
      name: 'partie-resume',
      component: PartiePage,
      meta: { title: 'Partie' },
    },
    {
      path: '/tournois',
      name: 'tournois',
      component: TournoisPage,
      meta: { title: 'Tournois en cours' },
    },
    {
      path: '/tournois/termines',
      name: 'tournois-termines',
      component: TournoisPage,
      meta: { title: 'Tournois terminés' },
    },
    {
      path: '/tournoi/:id',
      name: 'tournoi',
      component: TournoiPage,
      props: true,
    },
    { path: '/joueur/:name', name: 'joueur', component: JoueurPage, props: true },
  ],
})

router.afterEach((to) => {
  if (to.name === 'scenarios' && typeof to.query.scenario === 'string') {
    return
  }
  if (to.name === 'match') {
    document.title = 'Match'
    return
  }
  if (to.name === 'match-cr') {
    document.title = pageTitle('Compte rendu')
    return
  }
  const suffix = to.meta.title
  if (typeof suffix === 'string') {
    document.title = pageTitle(suffix)
  }
})
