import { createApp } from 'vue'
import './style.css'
import App from './App.vue'
import { router } from '@/router'
import { refreshAuth } from '@/composables/useAuth'

void refreshAuth()

createApp(App).use(router).mount('#app')
