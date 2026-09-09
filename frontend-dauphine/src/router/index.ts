import { createRouter, createWebHistory } from 'vue-router'
import HomePage from '@/pages/HomePage.vue'

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
      meta: { inProgress: true },
    },
  ],
})

router.afterEach(() => {
  document.title = 'Le Dauphiné'
})
