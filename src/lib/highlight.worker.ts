// Tokenizes diff hunks for syntax colors away from the UI thread.
import { tokenizeHunks } from './highlight-core'
import type { Hunk } from '../types'

self.onmessage = async (e: MessageEvent<{ id: number; path: string; hunks: Hunk[] }>) => {
  const { id, path, hunks } = e.data
  const toks = await tokenizeHunks(path, hunks).catch(() => null)
  self.postMessage({ id, toks })
}
