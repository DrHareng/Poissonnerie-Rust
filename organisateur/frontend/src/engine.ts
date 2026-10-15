import init, {
  dispatch as dispatchWasm,
  export_workbook,
  inspect_tournament,
  preview_points,
  sectorials_json,
} from './pkg/organisateur.js'
import type { EngineOk, Sectorial, Tournament, View } from './types'

let starting: Promise<void> | null = null

export function ready() {
  if (!starting) starting = init().then(() => undefined)
  return starting
}

function parse<T>(raw: string): T {
  const value = JSON.parse(raw) as { ok: boolean; error?: string }
  if (!value.ok) throw new Error(value.error || 'Erreur')
  return value as T
}

export async function loadSectorials(): Promise<Sectorial[]> {
  await ready()
  return JSON.parse(sectorials_json()) as Sectorial[]
}

export async function openView(tournament: Tournament): Promise<View> {
  await ready()
  return parse<{ ok: true; view: View }>(inspect_tournament(JSON.stringify(tournament))).view
}

export async function dispatch(tournament: Tournament | null, action: object): Promise<EngineOk> {
  await ready()
  const state = tournament ? JSON.stringify(tournament) : undefined
  return parse<EngineOk>(dispatchWasm(state, JSON.stringify(action)))
}

export async function previewPoints(p1Op: number, p2Op: number) {
  await ready()
  return JSON.parse(preview_points(p1Op, p2Op)) as { p1: number; p2: number }
}

export function previewPointsSync(p1Op: number, p2Op: number) {
  return JSON.parse(preview_points(p1Op, p2Op)) as { p1: number; p2: number }
}

export async function workbook(tournament: Tournament) {
  await ready()
  return new Uint8Array(export_workbook(JSON.stringify(tournament)))
}

export function freshSeed() {
  const buf = new Uint32Array(2)
  crypto.getRandomValues(buf)
  return (BigInt(buf[0]) << 32n | BigInt(buf[1])).toString()
}
