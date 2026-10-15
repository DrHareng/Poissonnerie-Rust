import type { Database } from 'sql.js'
import scriptUrl from 'sql.js/dist/sql-wasm-browser.js?url'
import wasmUrl from 'sql.js/dist/sql-wasm-browser.wasm?url'
import type { Status, Tournament } from './types'

interface SqlModule {
  Database: new (data?: Uint8Array) => Database
}

type SqlStarter = (config?: { locateFile?: (file: string) => string }) => Promise<SqlModule>

function sqlStarter() {
  return (window as unknown as { initSqlJs?: SqlStarter }).initSqlJs
}

function loadSqlScript() {
  return new Promise<void>((resolve, reject) => {
    if (sqlStarter()) {
      resolve()
      return
    }
    const script = document.createElement('script')
    script.src = scriptUrl
    script.onload = () => resolve()
    script.onerror = () => reject(new Error('SQLite indisponible'))
    document.head.appendChild(script)
  })
}

export interface TournamentListItem {
  id: string
  name: string
  status: Status
  updated_at: number
  round: number
  rounds: number
  players: number
}

const IDB = 'organisateur'
const STORE = 'sqlite'

let database: Database | null = null
let opening: Promise<Database> | null = null

function openIndexedDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(IDB, 1)
    request.onupgradeneeded = () => {
      request.result.createObjectStore(STORE)
    }
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error)
  })
}

function idbGet(db: IDBDatabase): Promise<Uint8Array | undefined> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, 'readonly')
    const request = tx.objectStore(STORE).get('file')
    request.onsuccess = () => resolve(request.result as Uint8Array | undefined)
    request.onerror = () => reject(request.error)
  })
}

function idbSet(db: IDBDatabase, bytes: Uint8Array): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, 'readwrite')
    tx.objectStore(STORE).put(bytes, 'file')
    tx.oncomplete = () => resolve()
    tx.onerror = () => reject(tx.error)
  })
}

async function open(): Promise<Database> {
  if (database) return database
  if (!opening) {
    opening = (async () => {
      await loadSqlScript()
      const initSqlJs = sqlStarter()
      if (!initSqlJs) throw new Error('SQLite indisponible')
      const SQL = await initSqlJs({ locateFile: () => wasmUrl })
      const idb = await openIndexedDb()
      const existing = await idbGet(idb)
      const db = existing ? new SQL.Database(existing) : new SQL.Database()
      db.run(`
        CREATE TABLE IF NOT EXISTS tournaments (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          status TEXT NOT NULL,
          updated_at INTEGER NOT NULL,
          state_json TEXT NOT NULL
        );
      `)
      database = db
      return db
    })()
  }
  return opening
}

async function persist() {
  const db = await open()
  const idb = await openIndexedDb()
  await idbSet(idb, db.export())
}

export async function listTournaments(): Promise<TournamentListItem[]> {
  const db = await open()
  const stmt = db.prepare(
    'SELECT name, status, updated_at, state_json FROM tournaments ORDER BY updated_at DESC',
  )
  const rows: TournamentListItem[] = []
  while (stmt.step()) {
    const row = stmt.getAsObject()
    const state = JSON.parse(String(row.state_json)) as Tournament
    rows.push({
      id: state.id,
      name: state.name,
      status: state.status,
      updated_at: Number(row.updated_at),
      round: state.round,
      rounds: state.rounds,
      players: state.players.length,
    })
  }
  stmt.free()
  return rows
}

export async function loadTournament(id: string): Promise<Tournament | null> {
  const db = await open()
  const stmt = db.prepare('SELECT state_json FROM tournaments WHERE id = ?')
  stmt.bind([id])
  const found = stmt.step() ? (JSON.parse(String(stmt.getAsObject().state_json)) as Tournament) : null
  stmt.free()
  return found
}

export async function saveTournament(tournament: Tournament) {
  const db = await open()
  db.run(
    `INSERT INTO tournaments (id, name, status, updated_at, state_json)
     VALUES (?, ?, ?, ?, ?)
     ON CONFLICT(id) DO UPDATE SET
       name = excluded.name,
       status = excluded.status,
       updated_at = excluded.updated_at,
       state_json = excluded.state_json`,
    [tournament.id, tournament.name, tournament.status, Date.now(), JSON.stringify(tournament)],
  )
  await persist()
}

export async function deleteTournament(id: string) {
  const db = await open()
  db.run('DELETE FROM tournaments WHERE id = ?', [id])
  await persist()
}
