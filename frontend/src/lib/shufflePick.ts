/** Fisher-Yates style random pick without replacement. */
export function shufflePick<T>(items: T[], count: number): T[] {
  const pool = [...items]
  const picks: T[] = []
  const n = Math.min(count, pool.length)
  for (let i = 0; i < n; i++) {
    const index = Math.floor(Math.random() * pool.length)
    picks.push(pool.splice(index, 1)[0]!)
  }
  return picks
}
