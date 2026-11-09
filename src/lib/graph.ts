import type { Commit } from '../types'

export interface Seg {
  /** from column x1 at height y1 to column x2 at height y2; 0 = top, 1 = middle (the dot), 2 = bottom */
  x1: number
  y1: 0 | 1
  x2: number
  y2: 1 | 2
  color: number
}

export interface GraphRow {
  col: number
  color: number
  segs: Seg[]
  width: number
}

/**
 * Lays out commits (newest first) in columns. Each column remembers the sha it is waiting
 * for; a commit takes the column waiting for it, then hands that column to its first parent.
 */
export function layout(commits: Commit[]): GraphRow[] {
  const lanes: (string | null)[] = []
  const colors: number[] = []
  let nextColor = 0
  const open = () => {
    const k = lanes.indexOf(null)
    return k < 0 ? lanes.length : k
  }
  return commits.map((c) => {
    let col = lanes.indexOf(c.sha)
    if (col < 0) {
      col = open()
      lanes[col] = null
      colors[col] = nextColor++
    }
    const color = colors[col]
    const segs: Seg[] = []
    lanes.forEach((s, i) => {
      if (s === null) return
      // Every lane waiting for this commit bends into its dot; the others pass straight through.
      if (s === c.sha) segs.push({ x1: i, y1: 0, x2: col, y2: 1, color: colors[i] })
      else segs.push({ x1: i, y1: 0, x2: i, y2: 2, color: colors[i] })
    })
    lanes.forEach((s, i) => {
      if (s === c.sha) lanes[i] = null
    })
    const [first, ...rest] = c.parents
    if (first) {
      lanes[col] = first
      segs.push({ x1: col, y1: 1, x2: col, y2: 2, color })
    }
    for (const p of rest) {
      let k = lanes.indexOf(p)
      if (k < 0) {
        k = open()
        lanes[k] = p
        colors[k] = nextColor++
      }
      segs.push({ x1: col, y1: 1, x2: k, y2: 2, color: colors[k] })
    }
    while (lanes.length && lanes[lanes.length - 1] === null) lanes.pop()
    const width = Math.max(col + 1, lanes.length, ...segs.map((s) => Math.max(s.x1, s.x2) + 1))
    return { col, color, segs, width }
  })
}

export const LANE_COLORS = ['#2f6fdf', '#1d9e75', '#d85a30', '#9b59d0', '#c2185b', '#0f8fa8', '#b7791f', '#5c6bc0']
