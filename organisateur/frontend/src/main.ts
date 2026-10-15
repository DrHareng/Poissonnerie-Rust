import { createApp } from 'vue'
import App from './App.vue'
import { listenForHardwareBack } from './back'
import { router } from './router'
import './style.css'

createApp(App).use(router).mount('#app')
listenForHardwareBack()
