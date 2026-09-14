const SESSION_KEY = 'poissonnerie.native-session'

export function isNativeApp(): boolean {
  const capacitor = (window as Window & { Capacitor?: { isNativePlatform?: () => boolean } })
    .Capacitor
  return Boolean(capacitor?.isNativePlatform?.())
}

export function getNativeSession(): string | null {
  try {
    return localStorage.getItem(SESSION_KEY)
  } catch {
    return null
  }
}

export function setNativeSession(sessionId: string | null) {
  try {
    if (sessionId) {
      localStorage.setItem(SESSION_KEY, sessionId)
    } else {
      localStorage.removeItem(SESSION_KEY)
    }
  } catch {
    /* private mode */
  }
}

function sessionFromUrl(url: string): string | null {
  try {
    const parsed = new URL(url)
    return parsed.searchParams.get('session')
  } catch {
    const match = url.match(/[?&]session=([^&]+)/)
    return match?.[1] ? decodeURIComponent(match[1]) : null
  }
}

export async function initNativeApp(): Promise<void> {
  if (!isNativeApp()) return

  const { App } = await import('@capacitor/app')
  const { Browser } = await import('@capacitor/browser')

  const applySession = async (url: string) => {
    const sessionId = sessionFromUrl(url)
    if (!sessionId) return
    setNativeSession(sessionId)
    try {
      await Browser.close()
    } catch {
      /* already closed */
    }
    window.dispatchEvent(new Event('poissonnerie-native-auth'))
  }

  App.addListener('appUrlOpen', (event) => {
    void applySession(event.url)
  })

  const launch = await App.getLaunchUrl()
  if (launch?.url) {
    await applySession(launch.url)
  }
}

export async function openNativeDiscordLogin(authorizeUrl: string): Promise<void> {
  const { Browser } = await import('@capacitor/browser')
  await Browser.open({ url: authorizeUrl, windowName: '_self' })
}
