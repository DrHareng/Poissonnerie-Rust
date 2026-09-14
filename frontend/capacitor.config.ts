import type { CapacitorConfig } from '@capacitor/cli'

const config: CapacitorConfig = {
  appId: 'fr.poissonnerie.app',
  appName: 'Poissonnerie',
  webDir: 'dist',
  server: {
    androidScheme: 'https',
    cleartext: true,
    allowNavigation: [
      'http://51.255.44.29',
      'http://51.255.44.29/*',
      'http://127.0.0.1:*',
      'http://localhost:*',
      'https://discord.com',
      'https://*.discord.com',
    ],
  },
  plugins: {
    CapacitorHttp: {
      enabled: true,
    },
    CapacitorCookies: {
      enabled: true,
    },
  },
}

export default config
