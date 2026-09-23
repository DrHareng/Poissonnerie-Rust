import path from 'node:path'
import tailwindcss from '@tailwindcss/vite'
import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vite'

const capacitor = process.env.CAPACITOR === '1'
const APP_BASE = capacitor ? './' : '/infinity/'

export default defineConfig({
  base: APP_BASE,
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    host: true,
    proxy: {
      '/infinity/api': {
        target: 'http://127.0.0.1:3000',
        rewrite: (p) => p.replace(/^\/infinity/, ''),
      },
      // Même entrée locale que la prod (Nginx) : /dauphine → SPA Dauphiné (port 5174).
      '/dauphine': {
        target: 'http://127.0.0.1:5174',
        changeOrigin: true,
        ws: true,
      },
    },
  },
})
