<script setup lang="ts">
import { ref, watch } from 'vue'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

const model = defineModel<string>({ required: true })

const hue = ref(0)
const saturation = ref(0)
const value = ref(1)
const code = ref('#FFFFFF')
const invalid = ref(false)
const plane = ref<HTMLElement | null>(null)
let fromPicker = false

watch(
  model,
  (raw) => {
    const parsed = normalizeHex(raw)
    if (!parsed) return
    code.value = `#${parsed}`
    invalid.value = false
    if (fromPicker) return
    const hsv = rgbToHsv(hexToRgb(parsed))
    if (hsv.s > 0.001) hue.value = hsv.h
    saturation.value = hsv.s
    value.value = hsv.v
  },
  { immediate: true },
)

function commitPicker() {
  const next = rgbToHex(hsvToRgb(hue.value, saturation.value, value.value))
  fromPicker = true
  model.value = next
  code.value = `#${next}`
  invalid.value = false
  fromPicker = false
}

function onHue(event: Event) {
  hue.value = Number((event.target as HTMLInputElement).value)
  commitPicker()
}

function onPlanePointer(event: PointerEvent) {
  if (event.type === 'pointermove' && event.buttons === 0) return
  const target = event.currentTarget as HTMLElement
  if (event.type === 'pointerdown') target.setPointerCapture(event.pointerId)
  const rect = plane.value?.getBoundingClientRect()
  if (!rect || rect.width === 0 || rect.height === 0) return
  saturation.value = clamp((event.clientX - rect.left) / rect.width)
  value.value = 1 - clamp((event.clientY - rect.top) / rect.height)
  commitPicker()
}

function onCode(event: Event) {
  const raw = (event.target as HTMLInputElement).value
  code.value = raw
  const parsed = normalizeHex(raw)
  invalid.value = raw.trim() !== '' && raw.trim() !== '#' && !parsed
  if (parsed) model.value = parsed
}

function onCodeBlur() {
  const parsed = normalizeHex(model.value)
  if (!parsed) return
  code.value = `#${parsed}`
  invalid.value = false
}

function clamp(channel: number) {
  return Math.min(1, Math.max(0, channel))
}

function normalizeHex(raw: string) {
  const text = raw.trim().replace(/^#/, '')
  if (!/^[0-9a-fA-F]+$/.test(text)) return ''
  const expanded = text.length === 3 ? text.replace(/./g, (char) => char + char) : text
  return expanded.length === 6 ? expanded.toUpperCase() : ''
}

function hexToRgb(hex: string) {
  return {
    r: Number.parseInt(hex.slice(0, 2), 16),
    g: Number.parseInt(hex.slice(2, 4), 16),
    b: Number.parseInt(hex.slice(4, 6), 16),
  }
}

function rgbToHex(rgb: { r: number; g: number; b: number }) {
  return [rgb.r, rgb.g, rgb.b].map((channel) => channel.toString(16).padStart(2, '0')).join('').toUpperCase()
}

function rgbToHsv(rgb: { r: number; g: number; b: number }) {
  const r = rgb.r / 255
  const g = rgb.g / 255
  const b = rgb.b / 255
  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  const delta = max - min
  let h = 0
  if (delta !== 0) {
    if (max === r) h = ((g - b) / delta) % 6
    else if (max === g) h = (b - r) / delta + 2
    else h = (r - g) / delta + 4
    h *= 60
    if (h < 0) h += 360
  }
  return { h, s: max === 0 ? 0 : delta / max, v: max }
}

function hsvToRgb(h: number, s: number, v: number) {
  const sector = ((h % 360) + 360) % 360 / 60
  const c = v * s
  const x = c * (1 - Math.abs((sector % 2) - 1))
  const m = v - c
  const channels = [
    [c, x, 0],
    [x, c, 0],
    [0, c, x],
    [0, x, c],
    [x, 0, c],
    [c, 0, x],
  ][Math.floor(sector) % 6]
  return {
    r: Math.round((channels[0] + m) * 255),
    g: Math.round((channels[1] + m) * 255),
    b: Math.round((channels[2] + m) * 255),
  }
}
</script>

<template>
  <div class="space-y-2">
    <div
      ref="plane"
      class="relative h-36 w-full touch-none rounded-md"
      :style="{ backgroundColor: `hsl(${hue} 100% 50%)` }"
      @pointerdown="onPlanePointer"
      @pointermove="onPlanePointer"
    >
      <div class="pointer-events-none absolute inset-0 rounded-md bg-gradient-to-r from-white to-transparent" />
      <div class="pointer-events-none absolute inset-0 rounded-md bg-gradient-to-t from-black to-transparent" />
      <div
        class="pointer-events-none absolute size-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_rgba(0,0,0,0.7)]"
        :style="{ left: `${saturation * 100}%`, top: `${(1 - value) * 100}%` }"
      />
    </div>
    <input
      class="hue-range h-3 w-full cursor-pointer appearance-none rounded-full"
      type="range"
      min="0"
      max="360"
      step="1"
      aria-label="Teinte"
      :value="hue"
      @input="onHue"
    />
    <div class="flex items-center gap-2">
      <div
        class="size-8 shrink-0 rounded-md border border-border"
        :style="{ backgroundColor: `#${normalizeHex(model) || 'FFFFFF'}` }"
      />
      <div class="min-w-0 flex-1 space-y-1">
        <Label for="figurine-color-code">Code</Label>
        <Input
          id="figurine-color-code"
          :model-value="code"
          spellcheck="false"
          autocapitalize="characters"
          autocomplete="off"
          placeholder="#00B0F2"
          class="font-mono uppercase"
          :aria-invalid="invalid || undefined"
          @input="onCode"
          @blur="onCodeBlur"
        />
      </div>
    </div>
    <p v-if="invalid" class="text-sm text-destructive">Code couleur invalide. Utilise #RRGGBB.</p>
  </div>
</template>

<style scoped>
.hue-range {
  background: linear-gradient(to right, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00);
}

.hue-range::-webkit-slider-thumb {
  appearance: none;
  width: 14px;
  height: 14px;
  border: 2px solid #111;
  border-radius: 999px;
  background: #fff;
}

.hue-range::-moz-range-thumb {
  width: 14px;
  height: 14px;
  border: 2px solid #111;
  border-radius: 999px;
  background: #fff;
}
</style>
