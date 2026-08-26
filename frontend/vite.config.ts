import path from 'node:path'
import tailwindcss from '@tailwindcss/vite'
import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vite'

const APP_BASE = '/infinity/'

export default defineConfig({
  base: APP_BASE,
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    proxy: {
      // Même préfixe qu'en prod : /infinity/api → API Rust /api
      [`${APP_BASE}api`]: {
        target: 'http://127.0.0.1:3000',
        rewrite: (p) => p.replace(new RegExp(`^${APP_BASE.replace(/\/$/, '')}`), ''),
      },
    },
  },
})
