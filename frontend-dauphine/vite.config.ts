import path from 'node:path'
import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vite'

const APP_BASE = '/dauphine/'

export default defineConfig({
  base: APP_BASE,
  plugins: [vue()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 5174,
    proxy: {
      [`${APP_BASE}api`]: {
        target: 'http://127.0.0.1:3000',
        rewrite: (p) => p.replace(new RegExp(`^${APP_BASE.replace(/\/$/, '')}`), ''),
      },
    },
  },
})
