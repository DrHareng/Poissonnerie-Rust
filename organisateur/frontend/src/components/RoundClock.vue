<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import {
  alertsFor,
  cancelClockNotifications,
  clearClock,
  formatRemaining,
  joinMinutes,
  readClock,
  scheduleClockNotifications,
  splitMinutes,
  writeClock,
} from '@/clock'
import WheelPicker from '@/components/WheelPicker.vue'

const props = defineProps<{
  tournamentId: string
  round: number
  minutes: number
}>()

const emit = defineEmits<{
  duration: [minutes: number]
}>()

const dock = ref<HTMLElement | null>(null)
const spacer = ref(96)
const hours = ref(2)
const mins = ref(30)
const running = ref(false)
const paused = ref(false)
const finished = ref(false)
const open = ref(false)
const editing = ref(false)
const startedAt = ref(0)
const durationMinutes = ref(150)
const pausedRemaining = ref(0)
const now = ref(Date.now())
const notice = ref('')
const permissionNote = ref('')
let seen = new Set<number>()
let timer = 0
let observer: ResizeObserver | null = null

const active = computed(() => running.value || paused.value || finished.value)
const chosen = computed(() => joinMinutes(hours.value, mins.value))
const alertPreview = computed(() =>
  alertsFor(active.value ? durationMinutes.value : chosen.value)
    .map((alert) => alert.title)
    .join('|'),
)
const remainingMs = computed(() => {
  if (paused.value) return pausedRemaining.value
  return startedAt.value + durationMinutes.value * 60_000 - now.value
})
const remainingLabel = computed(() => formatRemaining(remainingMs.value))
const statusLabel = computed(() => {
  if (finished.value) return "C'est terminé !"
  if (paused.value) return 'En pause'
  return 'Temps restant'
})

function rememberPassed() {
  const left = paused.value ? pausedRemaining.value : startedAt.value + durationMinutes.value * 60_000 - Date.now()
  seen = new Set(
    alertsFor(durationMinutes.value)
      .filter((alert) => left <= alert.remaining * 60_000)
      .map((alert) => alert.remaining),
  )
}

function showStored(storedMinutes: number) {
  const parts = splitMinutes(storedMinutes)
  hours.value = parts.hours
  mins.value = parts.minutes
}

function persist(pausedRemainingMs?: number) {
  writeClock(props.tournamentId, {
    round: props.round,
    startedAt: startedAt.value,
    minutes: durationMinutes.value,
    pausedRemainingMs: pausedRemainingMs ?? null,
  })
}

function restore() {
  const stored = readClock(props.tournamentId)
  notice.value = ''
  open.value = false
  editing.value = false
  paused.value = false
  if (!stored || stored.round !== props.round) {
    showStored(props.minutes)
    running.value = false
    finished.value = false
    if (stored) {
      clearClock(props.tournamentId)
      void cancelClockNotifications()
    }
    return
  }
  now.value = Date.now()
  startedAt.value = stored.startedAt
  durationMinutes.value = stored.minutes
  showStored(stored.minutes)
  if (stored.pausedRemainingMs != null) {
    pausedRemaining.value = stored.pausedRemainingMs
    finished.value = stored.pausedRemainingMs <= 0
    paused.value = stored.pausedRemainingMs > 0
    running.value = false
    return
  }
  const left = stored.startedAt + stored.minutes * 60_000 - Date.now()
  finished.value = left <= 0
  running.value = left > 0
  if (running.value) rememberPassed()
}

function toggle() {
  open.value = !open.value
  if (!open.value) editing.value = false
}

function showEditor() {
  showStored(durationMinutes.value)
  editing.value = true
}

function closeEditor() {
  showStored(durationMinutes.value)
  editing.value = false
}

async function launch() {
  const minutes = chosen.value
  if (!minutes) return
  const started = Date.now()
  now.value = started
  startedAt.value = started
  durationMinutes.value = minutes
  running.value = true
  paused.value = false
  finished.value = false
  editing.value = false
  notice.value = ''
  permissionNote.value = ''
  persist()
  rememberPassed()
  emit('duration', minutes)
  await arm(true)
}

async function pause() {
  const left = Math.max(0, startedAt.value + durationMinutes.value * 60_000 - Date.now())
  pausedRemaining.value = left
  paused.value = true
  running.value = false
  persist(left)
  try {
    await cancelClockNotifications()
  } catch {
    // The countdown stays frozen even if a notification cannot be cancelled.
  }
}

async function resume() {
  const left = pausedRemaining.value
  const started = Date.now() - (durationMinutes.value * 60_000 - left)
  now.value = Date.now()
  startedAt.value = started
  paused.value = false
  running.value = left > 0
  finished.value = left <= 0
  persist()
  rememberPassed()
  if (running.value) await arm(true)
}

async function reset() {
  showStored(durationMinutes.value)
  editing.value = false
  await launch()
}

async function arm(ask: boolean) {
  try {
    const granted = await scheduleClockNotifications(
      { round: props.round, startedAt: startedAt.value, minutes: durationMinutes.value },
      ask,
    )
    if (!granted && ask) permissionNote.value = 'Les notifications sont désactivées sur cet appareil.'
  } catch {
    if (ask) permissionNote.value = 'Les notifications sont désactivées sur cet appareil.'
  }
}

function tick() {
  now.value = Date.now()
  if (!running.value) return
  const left = remainingMs.value
  for (const alert of alertsFor(durationMinutes.value)) {
    if (seen.has(alert.remaining) || left > alert.remaining * 60_000) continue
    seen.add(alert.remaining)
    if (alert.remaining > 0) notice.value = alert.title
  }
  if (left <= 0) {
    running.value = false
    finished.value = true
    notice.value = ''
  }
}

onMounted(() => {
  restore()
  if (running.value) {
    void arm(false)
  }
  if (dock.value) {
    observer = new ResizeObserver(() => {
      spacer.value = (dock.value?.offsetHeight ?? 0) + 12
    })
    observer.observe(dock.value)
  }
  timer = window.setInterval(tick, 250)
})

onUnmounted(() => {
  window.clearInterval(timer)
  observer?.disconnect()
})

watch(() => [props.tournamentId, props.round] as const, restore)
</script>

<template>
  <div class="clock-spacer" :style="{ height: `${spacer}px` }" aria-hidden="true" />
  <Teleport to="body">
    <section ref="dock" class="clock-dock" :data-alerts="alertPreview">
      <div class="clock-dock-inner">
        <template v-if="active">
          <button
            class="clock-face"
            type="button"
            :aria-expanded="open"
            :aria-label="open ? 'Masquer les commandes' : 'Afficher les commandes'"
            @click="toggle"
          >
            <span class="clock-readout">
              <span v-if="!finished" class="muted">{{ statusLabel }}</span>
              <span v-if="finished" class="clock-done">C'est terminé !</span>
              <span v-else class="clock-time">{{ remainingLabel }}</span>
            </span>
            <span class="clock-chevron" :class="{ open }" aria-hidden="true" />
          </button>
          <div v-if="open" class="clock-actions">
            <button v-if="!finished" class="btn" type="button" @click="paused ? resume() : pause()">
              {{ paused ? 'Reprendre' : 'Pause' }}
            </button>
            <button class="btn" type="button" @click="reset">Reset</button>
            <button class="btn" type="button" @click="editing ? closeEditor() : showEditor()">
              {{ editing ? 'Annuler' : 'Modifier' }}
            </button>
          </div>
          <div v-if="open && editing" class="clock-launch">
            <WheelPicker v-model="hours" :min="0" :max="12" label="Heures" pad />
            <WheelPicker v-model="mins" :min="0" :max="59" label="Minutes" pad />
            <button class="btn primary" type="button" :disabled="!chosen" @click="launch">
              Appliquer
            </button>
          </div>
          <p v-if="notice" class="banner warn">{{ notice }}</p>
        </template>
        <template v-else>
          <div class="clock-launch">
            <WheelPicker v-model="hours" :min="0" :max="12" label="Heures" pad />
            <WheelPicker v-model="mins" :min="0" :max="59" label="Minutes" pad />
            <button class="btn primary" type="button" :disabled="!chosen" @click="launch">
              Lancer la clock
            </button>
          </div>
        </template>
        <p v-if="permissionNote" class="banner warn">{{ permissionNote }}</p>
      </div>
    </section>
  </Teleport>
</template>
