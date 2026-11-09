import type { DiffLine, FileDiff, Hunk } from '../types'

export interface Row {
  /** index of the line in hunk.lines */
  i: number
  t: ' ' | '+' | '-'
  text: string
  old: number | null
  new: number | null
  /** changed character range inside this line, when it pairs with a line on the other side */
  word: [number, number] | null
}

/** Line numbers plus intra-line ("word") change ranges for one hunk. */
export function hunkRows(h: Hunk): Row[] {
  let o = h.old_start
  let n = h.new_start
  const rows: Row[] = h.lines.map((l: DiffLine, i) => {
    const r: Row = { i, t: l.t, text: l.text, old: l.t === '+' ? null : o, new: l.t === '-' ? null : n, word: null }
    if (l.t !== '+') o++
    if (l.t !== '-') n++
    return r
  })
  for (let i = 0; i < rows.length; ) {
    if (rows[i].t !== '-') {
      i++
      continue
    }
    let j = i
    while (j < rows.length && rows[j].t === '-') j++
    let k = j
    while (k < rows.length && rows[k].t === '+') k++
    for (let q = 0; q < Math.min(j - i, k - j); q++) {
      const w = wordRange(rows[i + q].text, rows[j + q].text)
      if (w) {
        rows[i + q].word = w[0]
        rows[j + q].word = w[1]
      }
    }
    i = Math.max(k, i + 1)
  }
  return rows
}

/** Common prefix/suffix trim. Null when most of the line changed (then a highlight just adds noise). */
export function wordRange(a: string, b: string): [[number, number], [number, number]] | null {
  let p = 0
  while (p < a.length && p < b.length && a[p] === b[p]) p++
  let s = 0
  while (s < a.length - p && s < b.length - p && a[a.length - 1 - s] === b[b.length - 1 - s]) s++
  const ca = a.length - p - s
  const cb = b.length - p - s
  if (ca === 0 && cb === 0) return null
  if (Math.max(ca, cb) > 0.7 * Math.max(a.length, b.length)) return null
  return [
    [p, a.length - s],
    [p, b.length - s],
  ]
}

/** Side-by-side rows: deletions on the left, additions on the right, paired in order. */
export function sideRows(rows: Row[]): [Row | null, Row | null][] {
  const out: [Row | null, Row | null][] = []
  for (let i = 0; i < rows.length; ) {
    if (rows[i].t === ' ') {
      out.push([rows[i], rows[i]])
      i++
      continue
    }
    let j = i
    while (j < rows.length && rows[j].t === '-') j++
    let k = j
    while (k < rows.length && rows[k].t === '+') k++
    for (let q = 0; q < Math.max(j - i, k - j); q++) out.push([q < j - i ? rows[i + q] : null, q < k - j ? rows[j + q] : null])
    i = k
  }
  return out
}

export function stats(d: FileDiff | null): [number, number] {
  let a = 0
  let r = 0
  for (const h of d?.hunks ?? [])
    for (const l of h.lines) {
      if (l.t === '+') a++
      else if (l.t === '-') r++
    }
  return [a, r]
}

/** Short fingerprint of a diff, so "已看" can be cleared when the file changes again. */
export function diffHash(d: FileDiff): string {
  let h = 2166136261
  for (const hk of d.hunks)
    for (const l of hk.lines) {
      const s = l.t + l.text
      for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619)
      h = Math.imul(h ^ 10, 16777619)
    }
  return (h >>> 0).toString(36)
}

export const totalLines = (d: FileDiff) => d.hunks.reduce((s, h) => s + h.lines.length, 0)
