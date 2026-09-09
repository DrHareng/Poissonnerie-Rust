/// <reference types="vite/client" />

export {}

declare module 'vue-router' {
  interface RouteMeta {
    /** Fond « en construction » à la place du parchemin nu. */
    inProgress?: boolean
  }
}
