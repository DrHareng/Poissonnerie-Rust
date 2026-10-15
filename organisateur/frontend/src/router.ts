import { createRouter, createWebHashHistory } from 'vue-router'
import ListPage from './pages/ListPage.vue'
import TournamentPage from './pages/TournamentPage.vue'

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', component: ListPage },
    { path: '/t/:id', component: TournamentPage },
  ],
})
