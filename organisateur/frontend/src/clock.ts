import { Capacitor } from '@capacitor/core'
import { LocalNotifications } from '@capacitor/local-notifications'

export const DEFAULT_ROUND_MINUTES = 150

export const CLOCK_ALERTS = [
  { remaining: 90, title: '90 minutes restantes' },
  { remaining: 60, title: '60 minutes restantes' },
  { remaining: 30, title: '30 minutes restantes' },
  { remaining: 15, title: '15 minutes restantes' },
  { remaining: 5, title: '5 minutes restantes' },
  { remaining: 0, title: "C'est terminé !" },
] as const

const CHANNEL_ID = 'round-clock'

export interface StoredClock {
  round: number
  startedAt: number
  minutes: number
  pausedRemainingMs?: number | null
}

export function alertsFor(durationMinutes: number) {
  return CLOCK_ALERTS.filter((alert) => alert.remaining < durationMinutes)
}

export function splitMinutes(total: number) {
  const safe = Number.isFinite(total) && total > 0 ? Math.trunc(total) : DEFAULT_ROUND_MINUTES
  return { hours: Math.floor(safe / 60), minutes: safe % 60 }
}

export function joinMinutes(hours: number, minutes: number) {
  const h = Number.isFinite(hours) ? Math.trunc(hours) : -1
  const m = Number.isFinite(minutes) ? Math.trunc(minutes) : -1
  if (h < 0 || h > 12 || m < 0 || m > 59) return 0
  const total = h * 60 + m
  return total >= 1 && total <= 12 * 60 ? total : 0
}

export function minutesToTime(total: number) {
  const parts = splitMinutes(total)
  return `${String(parts.hours).padStart(2, '0')}:${String(parts.minutes).padStart(2, '0')}`
}

export function timeToMinutes(value: string) {
  const match = /^(\d{1,2}):(\d{2})/.exec(value.trim())
  if (!match) return 0
  return joinMinutes(Number(match[1]), Number(match[2]))
}

export function formatRemaining(ms: number) {
  const total = Math.max(0, Math.ceil(ms / 1000))
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const seconds = total % 60
  return `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
}

function storageKey(id: string) {
  return `organisateur.clock.${id}`
}

export function readClock(id: string): StoredClock | null {
  const raw = localStorage.getItem(storageKey(id))
  if (!raw) return null
  try {
    const value = JSON.parse(raw) as StoredClock
    if (!value || value.round < 1 || typeof value.startedAt !== 'number' || value.minutes < 1) return null
    if (value.pausedRemainingMs != null && !Number.isFinite(value.pausedRemainingMs)) return null
    return value
  } catch {
    return null
  }
}

export function writeClock(id: string, clock: StoredClock) {
  localStorage.setItem(storageKey(id), JSON.stringify(clock))
}

export function clearClock(id: string) {
  localStorage.removeItem(storageKey(id))
}

export async function forgetClock(id: string) {
  const stored = readClock(id)
  clearClock(id)
  if (!stored) return
  try {
    await cancelClockNotifications()
  } catch {
    // The tournament is already gone from the clock store.
  }
}

function notificationId(round: number, remaining: number) {
  return round * 10 + CLOCK_ALERTS.findIndex((alert) => alert.remaining === remaining) + 1
}

export async function cancelClockNotifications() {
  const ids = []
  for (let round = 1; round <= 12; round += 1) {
    for (const alert of CLOCK_ALERTS) ids.push({ id: notificationId(round, alert.remaining) })
  }
  await LocalNotifications.cancel({ notifications: ids })
}

export async function scheduleClockNotifications(
  clock: StoredClock,
  ask: boolean,
): Promise<boolean> {
  const pending = alertsFor(clock.minutes)
    .map((alert) => ({
      alert,
      at: clock.startedAt + (clock.minutes - alert.remaining) * 60_000,
    }))
    .filter((item) => item.at > Date.now() + 250)

  await cancelClockNotifications()
  const permission = ask
    ? await LocalNotifications.requestPermissions()
    : await LocalNotifications.checkPermissions()
  const granted = permission.display === 'granted'
  if (Capacitor.getPlatform() === 'android') {
    await LocalNotifications.createChannel({
      id: CHANNEL_ID,
      name: 'Clock de ronde',
      description: 'Rappels pendant une ronde',
      importance: 5,
      visibility: 1,
      vibration: true,
    })
    const exact = await LocalNotifications.checkExactNotificationSetting()
    if (exact.exact_alarm !== 'granted' && ask) {
      await LocalNotifications.changeExactNotificationSetting()
    }
  }
  if (granted && pending.length > 0) {
    await LocalNotifications.schedule({
      notifications: pending.map((item) => ({
        id: notificationId(clock.round, item.alert.remaining),
        title: item.alert.title,
        body: `Ronde ${clock.round}`,
        channelId: CHANNEL_ID,
        schedule: { at: new Date(item.at), allowWhileIdle: true },
      })),
    })
  }
  return granted
}
