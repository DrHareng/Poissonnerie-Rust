import { createRouter, createWebHistory } from 'vue-router'
import HomePage from '@/pages/HomePage.vue'
import PlaceholderPage from '@/pages/PlaceholderPage.vue'

export const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  scrollBehavior() {
    return { top: 0 }
  },
  routes: [
    {
      path: '/',
      name: 'accueil',
      component: HomePage,
      meta: { title: 'Accueil' },
    },
    {
      path: '/editions',
      name: 'editions',
      component: PlaceholderPage,
      meta: { title: 'Éditions' },
    },
    {
      path: '/inscription',
      name: 'inscription',
      component: PlaceholderPage,
      meta: { title: 'Inscriptions' },
    },
    {
      path: '/live',
      name: 'live',
      component: PlaceholderPage,
      meta: { title: 'Live' },
    },
  ],
})

router.afterEach((to) => {
  const title = typeof to.meta.title === 'string' ? to.meta.title : null
  document.title = title ? `${title} — Le Dauphiné` : 'Le Dauphiné'
})
