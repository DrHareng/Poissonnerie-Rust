import { createRouter, createWebHistory } from 'vue-router'
import ClassementPage from '@/pages/ClassementPage.vue'
import JoueurPage from '@/pages/JoueurPage.vue'
import MatchsPage from '@/pages/MatchsPage.vue'
import SectoriellesPage from '@/pages/SectoriellesPage.vue'
import SectoriellePage from '@/pages/SectoriellePage.vue'
import ScenariosPage from '@/pages/ScenariosPage.vue'
import TournoisPage from '@/pages/TournoisPage.vue'
import TournoiPage from '@/pages/TournoiPage.vue'
import PartiePage from '@/pages/PartiePage.vue'
import { pageTitle } from '@/lib/pageTitle'

export const router = createRouter({
  history: createWebHistory(),
  scrollBehavior(to) {
    if (to.hash) {
      return { el: to.hash, top: 88, behavior: 'smooth' }
    }
    return { top: 0 }
  },
  routes: [
    { path: '/', redirect: '/classement' },
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
      path: '/partie',
      name: 'partie',
      component: PartiePage,
      meta: { title: 'Partie' },
    },
    {
      path: '/tournois',
      name: 'tournois',
      component: TournoisPage,
      meta: { title: 'Tournois' },
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
  const suffix = to.meta.title
  if (typeof suffix === 'string') {
    document.title = pageTitle(suffix)
  }
})
