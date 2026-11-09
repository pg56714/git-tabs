// Main-thread side of syntax highlighting: sends hunks to a worker and splits tokens for
// word-level marks. The tokenizer itself lives in highlight-core.ts.

import type { Hunk } from '../types'
import type { Tok } from './highlight-core'

export type { Tok } from './highlight-core'

let worker: Worker | null = null
let seq = 0
const waiting = new Map<number, (t: Tok[][][] | null) => void>()

function getWorker(): Worker {
  if (!worker) {
    worker = new Worker(new URL('./highlight.worker.ts', import.meta.url), { type: 'module' })
    worker.onmessage = (e: MessageEvent<{ id: number; toks: Tok[][][] | null }>) => {
      waiting.get(e.data.id)?.(e.data.toks)
      waiting.delete(e.data.id)
    }
  }
  return worker
}

/** Tokens per line of each hunk, computed off the main thread (null when the language is unknown). */
export function highlightHunks(path: string, hunks: Hunk[]): Promise<Tok[][][] | null> {
  const id = ++seq
  return new Promise((resolve) => {
    waiting.set(id, resolve)
    // Only plain data crosses into the worker (the hunks may be reactive proxies).
    getWorker().postMessage({ id, path, hunks: hunks.map((h) => ({ lines: h.lines.map((l) => ({ t: l.t, text: l.text })) })) })
  })
}

/** Splits tokens at a [start, end) character range so the range can get its own background. */
export function splitTokens(toks: Tok[], range: [number, number] | null): (Tok & { mark?: boolean })[] {
  if (!range) return toks
  const out: (Tok & { mark?: boolean })[] = []
  let pos = 0
  for (const t of toks) {
    const end = pos + t.text.length
    const cuts = [pos, Math.min(Math.max(range[0], pos), end), Math.min(Math.max(range[1], pos), end), end]
    for (let i = 0; i < 3; i++) {
      if (cuts[i + 1] > cuts[i]) out.push({ ...t, text: t.text.slice(cuts[i] - pos, cuts[i + 1] - pos), mark: i === 1 })
    }
    pos = end
  }
  return out
}
